//! PostgreSQL persistence: the keep-list, and the history of every sync.
//!
//! The keep-list is keyed on the immutable GitHub account id, never on the login,
//! so protection survives an account rename.

use std::env;

use anyhow::{Context, Result, anyhow};
use postgres::types::ToSql;
use postgres::{Client, Config, NoTls, Transaction};

use crate::model::User;

/// Environment variable holding the connection string.
pub const URL_VAR: &str = "DATABASE_URL";

/// Directory holding the local PostgreSQL socket.
const SOCKET_DIR: &str = "/run/postgresql";

/// Parses a connection string, defaulting to the local socket when it names no host.
///
/// `psql` reads `postgresql:///goodbye` as "connect over the local socket", but
/// that fallback lives in libpq, not in this driver, which instead refuses with
/// "both host and hostaddr are missing". Since the short form is the natural one
/// to write, it is honoured here rather than documented as a trap.
///
/// # Errors
///
/// Fails when the string is not a valid connection string.
pub fn connection_config(url: &str) -> Result<Config> {
    let mut config: Config = url
        .parse()
        .with_context(|| format!("{URL_VAR} is not a valid connection string"))?;

    if config.get_hosts().is_empty() {
        config.host_path(SOCKET_DIR);
    }

    Ok(config)
}

/// Schema applied on every start. Idempotent, so there is no migration step.
const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS keep_list (
        id       BIGINT PRIMARY KEY,
        login    TEXT        NOT NULL,
        note     TEXT,
        added_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );

    CREATE TABLE IF NOT EXISTS sync_run (
        id       BIGSERIAL   PRIMARY KEY,
        taken_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );

    CREATE TABLE IF NOT EXISTS sync_member (
        run_id    BIGINT NOT NULL REFERENCES sync_run(id) ON DELETE CASCADE,
        direction TEXT   NOT NULL CHECK (direction IN ('following', 'follower')),
        user_id   BIGINT NOT NULL,
        login     TEXT   NOT NULL,
        PRIMARY KEY (run_id, direction, user_id)
    );
";

/// Splits accounts into parallel arrays, which is how they are handed to
/// PostgreSQL so that a whole batch travels in one statement rather than a loop
/// of round trips.
fn columns(users: &[User]) -> (Vec<i64>, Vec<String>) {
    users
        .iter()
        .map(|user| (user.id, user.login.clone()))
        .unzip()
}

/// An open connection to the store.
pub struct Store {
    client: Client,
}

impl Store {
    /// Connects using `DATABASE_URL` and applies the schema.
    ///
    /// # Errors
    ///
    /// Fails when `DATABASE_URL` is unset, when the server cannot be reached, or
    /// when the schema cannot be applied. Any of those leaves the keep-list
    /// unavailable, which the caller must treat as a reason to disable
    /// unfollowing.
    pub fn connect() -> Result<Self> {
        // Deliberately not `with_context`, whose source would append a bare
        // "environment variable not found" and leave the banner reading as two
        // half sentences joined by a colon.
        let url = env::var(URL_VAR)
            .map_err(|_| anyhow!("{URL_VAR} is not set. See the README for the setup steps"))?;

        let client = connection_config(&url)?
            .connect(NoTls)
            .with_context(|| format!("could not connect to PostgreSQL using {URL_VAR}"))?;

        let mut store = Self { client };
        store
            .client
            .batch_execute(SCHEMA)
            .context("connected to PostgreSQL but could not apply the schema")?;

        Ok(store)
    }

    /// Every account id currently on the keep-list.
    ///
    /// Ids are enough, because the accounts themselves are always drawn from the
    /// live sync rather than from this table, which keeps logins fresh.
    ///
    /// # Errors
    ///
    /// Fails when the query cannot be executed.
    pub fn keep_list(&mut self) -> Result<Vec<i64>> {
        let rows = self
            .client
            .query("SELECT id FROM keep_list", &[])
            .context("could not read the keep-list")?;

        Ok(rows.iter().map(|row| row.get(0)).collect())
    }

    /// Adds accounts to the keep-list, refreshing the cached login of any that
    /// were already there.
    ///
    /// # Errors
    ///
    /// Fails when the statement cannot be executed.
    pub fn keep(&mut self, users: &[User]) -> Result<()> {
        let (ids, logins) = columns(users);
        let params: [&(dyn ToSql + Sync); 2] = [&ids, &logins];

        self.client
            .execute(
                "INSERT INTO keep_list (id, login)
                 SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[])
                 ON CONFLICT (id) DO UPDATE SET login = EXCLUDED.login",
                &params,
            )
            .context("could not add to the keep-list")?;

        Ok(())
    }

    /// Removes accounts from the keep-list, returning them to the unfollow bucket.
    ///
    /// # Errors
    ///
    /// Fails when the statement cannot be executed.
    pub fn unkeep(&mut self, ids: &[i64]) -> Result<()> {
        self.client
            .execute(
                "DELETE FROM keep_list WHERE id = ANY($1::BIGINT[])",
                &[&ids],
            )
            .context("could not remove from the keep-list")?;

        Ok(())
    }

    /// Records one sync, so follow-graph trends can be reported later.
    ///
    /// Written in a transaction, so a run is either fully recorded or not
    /// recorded at all, and history never contains a half-written sync.
    ///
    /// # Errors
    ///
    /// Fails when any statement in the transaction cannot be executed.
    pub fn record_sync(&mut self, following: &[User], followers: &[User]) -> Result<()> {
        let mut transaction = self
            .client
            .transaction()
            .context("could not open a transaction to record the sync")?;

        let run_id: i64 = transaction
            .query_one("INSERT INTO sync_run DEFAULT VALUES RETURNING id", &[])
            .context("could not open a sync record")?
            .get(0);

        insert_members(&mut transaction, run_id, "following", following)?;
        insert_members(&mut transaction, run_id, "follower", followers)?;

        transaction
            .commit()
            .context("could not commit the sync record")
    }
}

/// Writes one side of a sync. Shared by both directions so the insert exists once.
fn insert_members(
    transaction: &mut Transaction<'_>,
    run_id: i64,
    direction: &str,
    users: &[User],
) -> Result<()> {
    let (ids, logins) = columns(users);
    let params: [&(dyn ToSql + Sync); 4] = [&run_id, &direction, &ids, &logins];

    transaction
        .execute(
            "INSERT INTO sync_member (run_id, direction, user_id, login)
             SELECT $1, $2, * FROM UNNEST($3::BIGINT[], $4::TEXT[])",
            &params,
        )
        .with_context(|| format!("could not record the {direction} side of the sync"))?;

    Ok(())
}
