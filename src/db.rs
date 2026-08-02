//! PostgreSQL persistence: the keep-list, and the history of every sync.
//!
//! The keep-list is keyed on the immutable GitHub account id, never on the login,
//! so protection survives an account rename.

use std::env;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow};
use postgres::types::ToSql;
use postgres::{Client, Config, NoTls, Transaction};

use crate::model::{DEFAULT_GRACE, Initiator, Relationship, Snapshot, User};

/// Environment variable holding the connection string.
pub const URL_VAR: &str = "DATABASE_URL";

/// Key the sweep window is stored under.
const GRACE_KEY: &str = "sweep_window_seconds";

/// Directory holding the local PostgreSQL socket.
const SOCKET_DIR: &str = "/run/postgresql";

/// Parses a connection string, defaulting to the local socket when it names no host.
///
/// `psql` reads `postgresql:///gitbye` as "connect over the local socket", but
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

    CREATE TABLE IF NOT EXISTS preference (
        key   TEXT   PRIMARY KEY,
        value BIGINT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sync_run (
        id       BIGSERIAL   PRIMARY KEY,
        taken_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );

    CREATE TABLE IF NOT EXISTS relationship (
        user_id            BIGINT PRIMARY KEY,
        login              TEXT NOT NULL,
        initiator          TEXT NOT NULL CHECK (initiator IN ('them', 'me', 'unknown')),
        they_followed_at   TIMESTAMPTZ,
        i_followed_at      TIMESTAMPTZ,
        they_unfollowed_at TIMESTAMPTZ
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

    /// The sweep window in force, falling back to the shipped default until one
    /// has been chosen.
    ///
    /// Stored rather than held per caller so the timer, the command line and the
    /// window cannot each be judging against a different figure.
    ///
    /// # Errors
    ///
    /// Fails when the query cannot be executed.
    pub fn grace(&mut self) -> Result<Duration> {
        let rows = self
            .client
            .query("SELECT value FROM preference WHERE key = $1", &[&GRACE_KEY])
            .context("could not read the sweep window")?;

        let Some(row) = rows.first() else {
            return Ok(DEFAULT_GRACE);
        };
        let seconds: i64 = row.get(0);

        // A negative or absurd row is corrupt rather than a setting, and the
        // default is a safer reading of it than a window of nothing.
        Ok(u64::try_from(seconds).map_or(DEFAULT_GRACE, Duration::from_secs))
    }

    /// Stores the sweep window, replacing any previous one.
    ///
    /// # Errors
    ///
    /// Fails when the statement cannot be executed.
    pub fn set_grace(&mut self, window: Duration) -> Result<()> {
        let seconds = i64::try_from(window.as_secs()).unwrap_or(i64::MAX);

        self.client
            .execute(
                "INSERT INTO preference (key, value) VALUES ($1, $2)
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
                &[&GRACE_KEY, &seconds],
            )
            .context("could not store the sweep window")?;

        Ok(())
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

    /// Everything known about who moved first, and when.
    ///
    /// # Errors
    ///
    /// Fails when the query cannot be executed.
    pub fn relationships(&mut self) -> Result<Vec<Relationship>> {
        let rows = self
            .client
            .query(
                "SELECT user_id, login, initiator, they_followed_at, i_followed_at,
                        they_unfollowed_at
                 FROM relationship",
                &[],
            )
            .context("could not read the relationship history")?;

        Ok(rows
            .iter()
            .map(|row| Relationship {
                user_id: row.get(0),
                login: row.get(1),
                initiator: Initiator::from_label(row.get(2)),
                they_followed_at: row.get(3),
                i_followed_at: row.get(4),
                they_unfollowed_at: row.get(5),
            })
            .collect())
    }

    /// Writes the updated history back, in one statement.
    ///
    /// # Errors
    ///
    /// Fails when the statement cannot be executed.
    pub fn save_relationships(&mut self, updated: &[Relationship]) -> Result<()> {
        if updated.is_empty() {
            return Ok(());
        }

        let ids: Vec<i64> = updated.iter().map(|entry| entry.user_id).collect();
        let logins: Vec<String> = updated.iter().map(|entry| entry.login.clone()).collect();
        let initiators: Vec<String> = updated
            .iter()
            .map(|entry| entry.initiator.label().to_owned())
            .collect();
        let followed: Vec<Option<SystemTime>> =
            updated.iter().map(|entry| entry.they_followed_at).collect();
        let returned: Vec<Option<SystemTime>> =
            updated.iter().map(|entry| entry.i_followed_at).collect();
        let left: Vec<Option<SystemTime>> = updated
            .iter()
            .map(|entry| entry.they_unfollowed_at)
            .collect();

        let params: [&(dyn ToSql + Sync); 6] =
            [&ids, &logins, &initiators, &followed, &returned, &left];

        self.client
            .execute(
                "INSERT INTO relationship
                     (user_id, login, initiator, they_followed_at, i_followed_at,
                      they_unfollowed_at)
                 SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TEXT[],
                                      $4::TIMESTAMPTZ[], $5::TIMESTAMPTZ[],
                                      $6::TIMESTAMPTZ[])
                 ON CONFLICT (user_id) DO UPDATE SET
                     login              = EXCLUDED.login,
                     initiator          = EXCLUDED.initiator,
                     they_followed_at   = EXCLUDED.they_followed_at,
                     i_followed_at      = EXCLUDED.i_followed_at,
                     they_unfollowed_at = EXCLUDED.they_unfollowed_at",
                &params,
            )
            .context("could not write the relationship history")?;

        Ok(())
    }

    /// Every recorded sync, oldest first, reduced to its two counts.
    ///
    /// # Errors
    ///
    /// Fails when the query cannot be executed.
    pub fn history(&mut self) -> Result<Vec<Snapshot>> {
        let rows = self
            .client
            .query(
                "SELECT run.taken_at,
                        count(*) FILTER (WHERE member.direction = 'following') AS following,
                        count(*) FILTER (WHERE member.direction = 'follower')  AS followers
                 FROM sync_run run
                 JOIN sync_member member ON member.run_id = run.id
                 GROUP BY run.id, run.taken_at
                 ORDER BY run.taken_at",
                &[],
            )
            .context("could not read the sync history")?;

        Ok(rows
            .iter()
            .map(|row| Snapshot {
                taken_at: row.get(0),
                // Counts come back as BIGINT and cannot be negative, so the
                // conversion is total rather than lossy.
                following: usize::try_from(row.get::<_, i64>(1)).unwrap_or(0),
                followers: usize::try_from(row.get::<_, i64>(2)).unwrap_or(0),
            })
            .collect())
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
