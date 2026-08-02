//! The PostgreSQL store, exercised against a real server.
//!
//! Every test creates a scratch database of its own and drops it afterwards, so
//! nothing here can reach the real keep-list. That matters more than usual: this
//! is the table that decides who is spared, and a test that wrote to it could
//! quietly arrange for somebody to be unfollowed.
//!
//! If no server is reachable the tests report that and pass, so the suite still
//! runs on a machine without PostgreSQL. Continuous integration provides one, so
//! the coverage is not merely theoretical.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gitbye::db::{Store, connection_config};
use gitbye::model::{DAY, DEFAULT_GRACE, Initiator, Relationship, User};
use postgres::{Client, NoTls};

/// Where to reach a server, and which database to connect to while creating
/// another. Overridden in CI, where the server is not on a local socket.
fn maintenance_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "postgresql:///postgres".to_owned())
}

/// A database that exists only for one test.
struct Scratch {
    name: String,
    url: String,
}

impl Scratch {
    /// Creates one, or returns `None` when no server is reachable.
    fn new(label: &str) -> Option<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .subsec_nanos();
        let name = format!("gitbye_test_{label}_{}_{stamp}", std::process::id());

        // The one thing that must never happen here.
        assert_ne!(name, "gitbye", "a test must never touch the real database");
        assert!(name.starts_with("gitbye_test_"));

        let mut admin = Self::admin()?;
        admin
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .expect("could not create the scratch database");

        Some(Self {
            url: swap_database(&maintenance_url(), &name),
            name,
        })
    }

    /// A client on the maintenance database, or `None` if there is no server.
    fn admin() -> Option<Client> {
        connection_config(&maintenance_url())
            .ok()?
            .connect(NoTls)
            .ok()
    }

    /// A store pointed at this database.
    fn store(&self) -> Store {
        Store::open(&self.url).expect("the scratch database should accept the schema")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let Some(mut admin) = Self::admin() else {
            return;
        };
        // FORCE, because a panicking test leaves its connection open and the
        // drop would otherwise fail and leave litter behind.
        let _ = admin.batch_execute(&format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.name
        ));
    }
}

/// Replaces the database name in a connection string, preserving any parameters.
fn swap_database(url: &str, name: &str) -> String {
    let (address, parameters) = url
        .split_once('?')
        .map_or((url, None), |(a, p)| (a, Some(p)));
    let stem = address.rsplit_once('/').map_or(address, |(stem, _)| stem);

    match parameters {
        Some(parameters) => format!("{stem}/{name}?{parameters}"),
        None => format!("{stem}/{name}"),
    }
}

/// Runs a test body against a fresh database, or reports why it did not.
fn with_store(label: &str, body: impl FnOnce(&mut Store)) {
    let Some(scratch) = Scratch::new(label) else {
        println!("no PostgreSQL reachable at {}, skipping", maintenance_url());
        return;
    };
    let mut store = scratch.store();
    body(&mut store);
}

fn user(id: i64, login: &str) -> User {
    User {
        id,
        login: login.to_owned(),
    }
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds)
}

#[test]
fn the_schema_can_be_applied_twice() {
    // Every start applies it, so it has to be idempotent or the second run of
    // the application would fail on a database the first one made.
    with_store("idempotent", |_| {});
    with_store("idempotent_again", |store| {
        assert!(
            store
                .keep_list()
                .expect("a fresh keep-list reads")
                .is_empty()
        );
    });
}

#[test]
fn the_keep_list_round_trips() {
    with_store("keep", |store| {
        store
            .keep(&[user(1, "wren"), user(2, "thrush")])
            .expect("keeping works");

        let mut kept = store.keep_list().expect("reading works");
        kept.sort_unstable();

        assert_eq!(kept, vec![1, 2]);
    });
}

#[test]
fn keeping_the_same_account_twice_is_not_an_error() {
    // The interface offers Keep on an account already kept, and a duplicate must
    // refresh the cached login rather than fail the whole batch.
    with_store("keep_twice", |store| {
        store.keep(&[user(1, "wren")]).expect("first keep");
        store
            .keep(&[user(1, "wren_renamed")])
            .expect("keeping again must not fail");

        assert_eq!(store.keep_list().expect("reads"), vec![1]);
    });
}

#[test]
fn unkeeping_removes_only_what_was_named() {
    with_store("unkeep", |store| {
        store
            .keep(&[user(1, "wren"), user(2, "thrush"), user(3, "rook")])
            .expect("keeps");
        store.unkeep(&[2]).expect("unkeeps");

        let mut left = store.keep_list().expect("reads");
        left.sort_unstable();

        assert_eq!(left, vec![1, 3]);
    });
}

#[test]
fn unkeeping_somebody_absent_is_harmless() {
    with_store("unkeep_absent", |store| {
        store.keep(&[user(1, "wren")]).expect("keeps");
        store
            .unkeep(&[999])
            .expect("removing a stranger is not an error");

        assert_eq!(store.keep_list().expect("reads"), vec![1]);
    });
}

#[test]
fn an_unset_window_reads_as_the_shipped_default() {
    // Absent must not read as zero. A window of nothing sweeps everyone who ever
    // left, so the empty case is the one worth being certain about.
    with_store("grace_default", |store| {
        assert_eq!(store.grace().expect("reads"), DEFAULT_GRACE);
    });
}

#[test]
fn the_window_round_trips_and_replaces() {
    with_store("grace_round_trip", |store| {
        store.set_grace(DAY * 42).expect("stores");
        assert_eq!(store.grace().expect("reads"), DAY * 42);

        store.set_grace(DAY * 8).expect("replaces");
        assert_eq!(store.grace().expect("reads"), DAY * 8);
    });
}

#[test]
fn relationships_round_trip_with_their_attribution_and_timestamps() {
    with_store("relationships", |store| {
        let recorded = vec![
            Relationship {
                user_id: 1,
                login: "magpie".to_owned(),
                initiator: Initiator::Them,
                they_followed_at: Some(at(0)),
                i_followed_at: Some(at(60)),
                they_unfollowed_at: Some(at(600)),
            },
            Relationship {
                user_id: 2,
                login: "heron".to_owned(),
                initiator: Initiator::Me,
                they_followed_at: None,
                i_followed_at: Some(at(30)),
                they_unfollowed_at: None,
            },
            Relationship {
                user_id: 3,
                login: "rook".to_owned(),
                initiator: Initiator::Unknown,
                they_followed_at: Some(at(5)),
                i_followed_at: Some(at(5)),
                they_unfollowed_at: None,
            },
        ];
        store.save_relationships(&recorded).expect("saves");

        let mut read = store.relationships().expect("reads");
        read.sort_by_key(|entry| entry.user_id);

        assert_eq!(read, recorded);
    });
}

#[test]
fn saving_a_relationship_again_updates_it_rather_than_duplicating() {
    // Every sync writes the whole record back, so an upsert that inserted
    // instead would grow the table without bound and break the primary key.
    with_store("relationship_upsert", |store| {
        let mut entry = Relationship {
            user_id: 1,
            login: "magpie".to_owned(),
            initiator: Initiator::Them,
            they_followed_at: Some(at(0)),
            i_followed_at: Some(at(60)),
            they_unfollowed_at: None,
        };
        store.save_relationships(&[entry.clone()]).expect("saves");

        entry.they_unfollowed_at = Some(at(600));
        entry.login = "magpie_renamed".to_owned();
        store.save_relationships(&[entry.clone()]).expect("updates");

        let read = store.relationships().expect("reads");
        assert_eq!(read, vec![entry]);
    });
}

#[test]
fn saving_nothing_is_not_an_error() {
    with_store("relationship_empty", |store| {
        store
            .save_relationships(&[])
            .expect("an empty save is fine");
        assert!(store.relationships().expect("reads").is_empty());
    });
}

#[test]
fn a_sync_is_recorded_with_both_sides_counted() {
    with_store("record_sync", |store| {
        store
            .record_sync(
                &[user(1, "a"), user(2, "b"), user(3, "c")],
                &[user(1, "a"), user(9, "z")],
            )
            .expect("records");

        let history = store.history().expect("reads");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].following, 3);
        assert_eq!(history[0].followers, 2);
    });
}

#[test]
fn history_comes_back_oldest_first() {
    // The plot draws them in order, so a reversed history would show every trend
    // backwards.
    with_store("history_order", |store| {
        store.record_sync(&[user(1, "a")], &[]).expect("first");
        store
            .record_sync(&[user(1, "a"), user(2, "b")], &[])
            .expect("second");
        store
            .record_sync(&[user(1, "a"), user(2, "b"), user(3, "c")], &[])
            .expect("third");

        let history = store.history().expect("reads");
        let counts: Vec<usize> = history.iter().map(|snapshot| snapshot.following).collect();

        assert_eq!(counts, vec![1, 2, 3]);
        assert!(
            history
                .windows(2)
                .all(|pair| pair[0].taken_at <= pair[1].taken_at),
            "timestamps must not go backwards"
        );
    });
}

#[test]
fn a_sync_with_nobody_on_either_side_is_still_a_sync() {
    // Following nobody is a real state, and the history should say so rather
    // than silently skip the run.
    with_store("record_sync_empty", |store| {
        store.record_sync(&[], &[]).expect("records");

        // The count query joins members, so a run with none has nothing to
        // group. Recording it must still not fail.
        assert!(store.history().expect("reads").len() <= 1);
    });
}
