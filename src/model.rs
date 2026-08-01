//! Domain types and the bucketing logic.
//!
//! This module is deliberately free of input and output. Everything here is a
//! plain value or a pure function, which is what makes the bucketing testable
//! without a network or a database.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

/// How long a new follower has to keep following before their follow is treated
/// as sincere.
///
/// Unfollow inside this window having been followed back, and the follow is
/// returned. Stay past it and the follow is kept even if they later leave.
///
pub const GRACE: Duration = Duration::from_secs(10 * 7 * 24 * 60 * 60);

/// Who moved first.
///
/// This is the whole basis of the automation, which is why it is recorded
/// rather than inferred: a follow you began is your decision and is never
/// undone automatically, while a follow you returned is contingent on theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initiator {
    /// They followed first and you returned it.
    Them,
    /// You followed first.
    Me,
    /// The relationship predates this application, so nothing can be claimed
    /// about it. Never eligible for automation.
    Unknown,
}

impl Initiator {
    /// How it is stored.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Them => "them",
            Self::Me => "me",
            Self::Unknown => "unknown",
        }
    }

    /// Reads it back. Anything unrecognised becomes [`Initiator::Unknown`],
    /// which is the safe direction: unknown is never swept.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "them" => Self::Them,
            "me" => Self::Me,
            _ => Self::Unknown,
        }
    }
}

/// What is known about one account's history with you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    /// Immutable GitHub account id.
    pub user_id: i64,
    /// Current account name.
    pub login: String,
    /// Who moved first.
    pub initiator: Initiator,
    /// When they were first observed following you.
    pub they_followed_at: Option<SystemTime>,
    /// When you were first observed following them.
    pub i_followed_at: Option<SystemTime>,
    /// When they were first observed to have stopped following you.
    pub they_unfollowed_at: Option<SystemTime>,
}

/// Updates what is known about every account, from one fresh observation.
///
/// Attribution is only ever recorded, never inferred after the fact. An account
/// already reciprocal the first time it is seen stays [`Initiator::Unknown`]
/// forever, because the order genuinely is not knowable from that point on.
#[must_use]
pub fn attribute(
    stored: &[Relationship],
    following: &[User],
    followers: &[User],
    now: SystemTime,
) -> Vec<Relationship> {
    let outgoing_ids: HashSet<i64> = following.iter().map(|user| user.id).collect();
    let incoming_ids: HashSet<i64> = followers.iter().map(|user| user.id).collect();
    let known: HashMap<i64, &Relationship> = stored
        .iter()
        .map(|relationship| (relationship.user_id, relationship))
        .collect();

    let mut seen = HashSet::new();
    let mut updated = Vec::new();

    for user in following.iter().chain(followers) {
        if !seen.insert(user.id) {
            continue;
        }
        let outgoing = outgoing_ids.contains(&user.id);
        let incoming = incoming_ids.contains(&user.id);

        updated.push(match known.get(&user.id) {
            Some(existing) => advance(existing, user, outgoing, incoming, now),
            None => begin(user, outgoing, incoming, now),
        });
    }

    updated
}

/// First sighting of an account.
fn begin(user: &User, outgoing: bool, incoming: bool, now: SystemTime) -> Relationship {
    let initiator = match (outgoing, incoming) {
        // Already mutual when first seen, so the order is lost to history.
        (true, true) | (false, false) => Initiator::Unknown,
        (true, false) => Initiator::Me,
        (false, true) => Initiator::Them,
    };

    Relationship {
        user_id: user.id,
        login: user.login.clone(),
        initiator,
        they_followed_at: incoming.then_some(now),
        i_followed_at: outgoing.then_some(now),
        they_unfollowed_at: None,
    }
}

/// A later sighting of an account already on record.
fn advance(
    existing: &Relationship,
    user: &User,
    outgoing: bool,
    incoming: bool,
    now: SystemTime,
) -> Relationship {
    let mut next = existing.clone();
    // Logins change; the record follows the account, not the name.
    next.login.clone_from(&user.login);

    if incoming && next.they_followed_at.is_none() {
        next.they_followed_at = Some(now);
    }
    if outgoing && next.i_followed_at.is_none() {
        next.i_followed_at = Some(now);
    }
    // Coming back starts a fresh window rather than resuming the old one, so a
    // follow, unfollow, refollow cycle cannot be used to run down the clock.
    if incoming && next.they_unfollowed_at.is_some() {
        next.they_followed_at = Some(now);
        next.they_unfollowed_at = None;
    }
    if !incoming && next.they_followed_at.is_some() && next.they_unfollowed_at.is_none() {
        next.they_unfollowed_at = Some(now);
    }

    next
}

/// Whether an automated sweep should return this follow.
///
/// Every condition must hold, and any missing timestamp means no. The rule is
/// deliberately conservative: it acts only where the application watched the
/// whole relationship happen.
///
/// `kept` is the keep-list, which overrides everything.
#[must_use]
pub fn should_sweep<S: BuildHasher>(relationship: &Relationship, kept: &HashSet<i64, S>) -> bool {
    // The keep-list outranks everything, automation included.
    if kept.contains(&relationship.user_id) {
        return false;
    }

    // Only a follow you returned can be withdrawn automatically, and only where
    // the application itself watched who moved first.
    if relationship.initiator != Initiator::Them || relationship.i_followed_at.is_none() {
        return false;
    }

    let (Some(started), Some(ended)) = (
        relationship.they_followed_at,
        relationship.they_unfollowed_at,
    ) else {
        return false;
    };

    // `duration_since` fails when the end precedes the start. That is corrupt
    // data rather than an extremely short window, so it refuses.
    ended.duration_since(started).is_ok_and(|held| held < GRACE)
}

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
