//! The unattended sweep.
//!
//! This is the only path in the application that can unfollow somebody with
//! nobody watching, so it is deliberately narrow. It refuses to run at all
//! unless the keep-list is readable, it acts only on relationships the
//! application observed from beginning to end, and it reports everything it did.

use std::collections::HashSet;
use std::hash::BuildHasher;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use crate::db::Store;
use crate::github::{self, Github};
use crate::model::{Relationship, User, attribute, should_sweep};

/// What a sweep did, or would have done.
pub struct Report {
    /// Whether this was a rehearsal.
    pub rehearsal: bool,
    /// Accounts the rule selected.
    pub selected: Vec<String>,
    /// Accounts successfully unfollowed. Empty after a rehearsal.
    pub unfollowed: Vec<String>,
    /// Accounts that could not be unfollowed, with the reason.
    pub failed: Vec<(String, String)>,
    /// How many relationships are on record but exempt because their beginning
    /// was never observed.
    pub exempt: usize,
}

impl Report {
    /// A human-readable account of the run, for a log or a terminal.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut lines = Vec::new();

        let verb = if self.rehearsal {
            "would unfollow"
        } else {
            "unfollowed"
        };
        lines.push(format!(
            "{verb} {} of {} selected",
            self.acted(),
            self.selected.len()
        ));

        for login in &self.selected {
            lines.push(format!("  {login}"));
        }
        for (login, reason) in &self.failed {
            lines.push(format!("  failed {login}: {reason}"));
        }
        if self.exempt > 0 {
            lines.push(format!(
                "{} relationships exempt: their beginning predates this record",
                self.exempt
            ));
        }

        lines.join("\n")
    }

    /// How many accounts were actually acted upon.
    fn acted(&self) -> usize {
        if self.rehearsal {
            self.selected.len()
        } else {
            self.unfollowed.len()
        }
    }
}

/// Runs one sweep.
///
/// With `rehearsal` set, nothing is written to GitHub and the report says what
/// would have happened. That is the mode to run from a timer first.
///
/// # Errors
///
/// Fails when the token cannot be borrowed, GitHub cannot be reached, or the
/// store is unavailable. A store failure is fatal here, unlike in the window:
/// with no keep-list there is no way to know who was meant to be spared, and
/// nobody is present to notice.
pub fn run(rehearsal: bool, window: Option<Duration>) -> Result<Report> {
    let github = Github::new(github::token()?)?;
    let mut store = Store::connect().context(
        "the sweep will not run without the keep-list, since it cannot ask anybody what to spare",
    )?;

    let following = github.following()?;
    let followers = github.followers()?;

    store.record_sync(&following, &followers)?;
    let known = store.relationships()?;
    let updated = attribute(&known, &following, &followers, SystemTime::now());
    store.save_relationships(&updated)?;

    // An explicit window governs this run only. Storing it here would turn
    // trying a figure out, which --dry-run exists for, into adopting it.
    let grace = match window {
        Some(chosen) => chosen,
        None => store.grace()?,
    };

    let kept: HashSet<i64> = store.keep_list()?.into_iter().collect();
    let still_following: HashSet<i64> = following.iter().map(|user| user.id).collect();

    let candidates = select(&updated, &kept, &still_following, grace);
    let exempt = updated
        .iter()
        .filter(|entry| entry.initiator == crate::model::Initiator::Unknown)
        .count();

    let mut report = Report {
        rehearsal,
        selected: candidates.iter().map(|user| user.login.clone()).collect(),
        unfollowed: Vec::new(),
        failed: Vec::new(),
        exempt,
    };

    if rehearsal {
        return Ok(report);
    }

    for user in candidates {
        match github.unfollow(&user.login) {
            Ok(()) => report.unfollowed.push(user.login),
            Err(error) => report.failed.push((user.login, format!("{error:#}"))),
        }
    }

    Ok(report)
}

/// Picks the accounts the rule selects, in a stable order.
///
/// Kept separate from the run so the decision is a pure function of recorded
/// state, with no network or clock of its own. Public for the same reason: this
/// is the code that decides who gets unfollowed unattended, so it is the code
/// the tests most need to reach.
#[must_use]
pub fn select<K: BuildHasher, F: BuildHasher>(
    updated: &[Relationship],
    kept: &HashSet<i64, K>,
    still_following: &HashSet<i64, F>,
    grace: Duration,
) -> Vec<User> {
    let mut chosen: Vec<User> = updated
        .iter()
        .filter(|entry| still_following.contains(&entry.user_id))
        .filter(|entry| should_sweep(entry, kept, grace))
        .map(|entry| User {
            id: entry.user_id,
            login: entry.login.clone(),
        })
        .collect();

    chosen.sort_by_key(|user| user.login.to_lowercase());
    chosen
}
