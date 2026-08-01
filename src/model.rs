//! Domain types and the bucketing logic.
//!
//! This module is deliberately free of input and output. Everything here is a
//! plain value or a pure function, which is what makes the bucketing testable
//! without a network or a database.

use std::collections::HashSet;

use serde::Deserialize;

/// A GitHub account.
///
/// Identified by `id` rather than `login`, because GitHub permits renames and the
/// login string travels with the account. Keying on a login would silently stop
/// protecting somebody the day they renamed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
    /// Immutable GitHub account id.
    pub id: i64,
    /// Current account name, refreshed on every sync.
    pub login: String,
}

/// The four mutually exclusive groups the follow graph sorts into.
///
/// Every account you follow lands in exactly one of `unreciprocated`, `keeping`
/// or `mutuals`. `fans` is drawn from your followers instead, so it never
/// overlaps the other three.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Buckets {
    /// Followed, does not follow back, not protected. The only bucket that
    /// unfollowing acts upon.
    pub unreciprocated: Vec<User>,
    /// Followed, does not follow back, protected by the keep-list.
    pub keeping: Vec<User>,
    /// Followed, follows back.
    pub mutuals: Vec<User>,
    /// Follows you, not followed by you.
    pub fans: Vec<User>,
}

/// What a background worker reports back to the interface thread.
///
/// The channel carrying these is the only synchronisation in the application.
/// There is no shared mutable state between the two threads, so there is no lock
/// to forget and no ordering to reason about.
pub enum Msg {
    /// A completed sync, carrying both sides of the graph and the keep-list.
    Synced {
        /// Accounts the user follows.
        following: Vec<User>,
        /// Accounts following the user.
        followers: Vec<User>,
        /// Ids currently on the keep-list, or `None` when the store could not be
        /// read. `None` must keep unfollowing disabled, because an empty
        /// keep-list and an unreadable one are indistinguishable in a set
        /// difference, and one of those means unfollowing protected accounts.
        keep: Option<Vec<i64>>,
    },
    /// The keep-list after it was changed, so buckets can be recomputed without
    /// another round trip to GitHub.
    KeepList(Vec<i64>),
    /// One step of a batch completed.
    Progress {
        /// Steps completed so far.
        done: usize,
        /// Steps in the batch.
        total: usize,
        /// Account the step acted on.
        login: String,
    },
    /// A batch ran to completion, successes and failures together.
    Finished {
        /// Accounts acted on successfully. Carried in full rather than counted,
        /// because offering to undo the batch means knowing exactly who it hit.
        done: Vec<User>,
        /// Accounts that failed, each with the reason.
        failed: Vec<(String, String)>,
    },
    /// A job could not run at all.
    Error(String),
}

/// Sorts by login, case-insensitively, so the interface ordering is stable and
/// the tests are deterministic.
fn sorted(mut users: Vec<User>) -> Vec<User> {
    users.sort_by_key(|user| user.login.to_lowercase());
    users
}

/// Sorts the follow graph into [`Buckets`].
///
/// `keep` holds the ids on the keep-list. Protection only applies to accounts
/// that do not follow back, so a keep-list entry for a mutual lies dormant until
/// that account stops reciprocating.
#[must_use]
pub fn bucket(following: &[User], followers: &[User], keep: &[i64]) -> Buckets {
    let follower_ids: HashSet<i64> = followers.iter().map(|user| user.id).collect();
    let following_ids: HashSet<i64> = following.iter().map(|user| user.id).collect();
    let kept: HashSet<i64> = keep.iter().copied().collect();

    let mut unreciprocated = Vec::new();
    let mut keeping = Vec::new();
    let mut mutuals = Vec::new();

    for user in following {
        // Matching on the pair keeps the three-way split flat. Reciprocation wins
        // over the keep-list, because protection is meaningless while an account
        // is following you back.
        let destination = match (follower_ids.contains(&user.id), kept.contains(&user.id)) {
            (true, _) => &mut mutuals,
            (false, true) => &mut keeping,
            (false, false) => &mut unreciprocated,
        };
        destination.push(user.clone());
    }

    let fans: Vec<User> = followers
        .iter()
        .filter(|user| !following_ids.contains(&user.id))
        .cloned()
        .collect();

    Buckets {
        unreciprocated: sorted(unreciprocated),
        keeping: sorted(keeping),
        mutuals: sorted(mutuals),
        fans: sorted(fans),
    }
}
