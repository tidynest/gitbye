//! Tests for recording who moved first.
//!
//! Attribution is what makes the automated sweep safe, so the cases that must
//! stay [`Initiator::Unknown`] matter as much as the ones that resolve.

use std::time::{Duration, SystemTime};

use gitbye::model::{Initiator, Relationship, User, attribute};

fn origin() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn later(days: u64) -> SystemTime {
    origin() + Duration::from_secs(days * 24 * 60 * 60)
}

fn user(id: i64, login: &str) -> User {
    User {
        id,
        login: login.to_owned(),
    }
}

/// An account already on record, unrelated to any test's subject.
///
/// Its only job is to make the record non-empty, so a newly seen account counts
/// as having been watched to appear rather than merely found on the first sync.
/// It never appears in the following or follower lists, so it never comes back
/// in the results.
fn established() -> Vec<Relationship> {
    vec![Relationship {
        user_id: 999,
        login: "already-known".to_owned(),
        initiator: Initiator::Unknown,
        they_followed_at: Some(origin()),
        i_followed_at: None,
        they_unfollowed_at: None,
    }]
}

fn only(updated: Vec<Relationship>) -> Relationship {
    assert_eq!(updated.len(), 1, "expected exactly one account on record");
    updated.into_iter().next().expect("one account")
}

#[test]
fn the_opening_graph_is_entirely_unknown() {
    let following = [user(1, "one-sided"), user(2, "mutual")];
    let followers = [user(2, "mutual"), user(3, "fan")];

    let updated = attribute(&[], &following, &followers, origin());

    assert_eq!(updated.len(), 3);
    assert!(
        updated
            .iter()
            .all(|entry| entry.initiator == Initiator::Unknown),
        "nothing about a graph that already existed was observed, so nothing may be claimed"
    );
}

#[test]
fn a_new_follower_you_do_not_follow_is_attributed_to_them() {
    let them = [user(1, "them")];

    let record = only(attribute(&established(), &[], &them, origin()));

    assert_eq!(record.initiator, Initiator::Them);
    assert_eq!(record.they_followed_at, Some(origin()));
    assert_eq!(record.i_followed_at, None);
}

#[test]
fn an_account_you_follow_first_is_attributed_to_you() {
    let mine = [user(2, "mine")];

    let record = only(attribute(&established(), &mine, &[], origin()));

    assert_eq!(record.initiator, Initiator::Me);
    assert_eq!(record.i_followed_at, Some(origin()));
}

#[test]
fn an_account_already_mutual_when_first_seen_stays_unknown_forever() {
    let both = [user(3, "old-friend")];

    let first = only(attribute(&established(), &both, &both, origin()));
    assert_eq!(first.initiator, Initiator::Unknown);

    let second = only(attribute(&[first], &both, &both, later(30)));
    assert_eq!(
        second.initiator,
        Initiator::Unknown,
        "an unobserved beginning can never be recovered"
    );
}

#[test]
fn following_back_records_the_time_without_changing_who_began_it() {
    let them = [user(1, "them")];

    let day_one = only(attribute(&established(), &[], &them, origin()));
    let day_two = only(attribute(&[day_one], &them, &them, later(1)));

    assert_eq!(day_two.initiator, Initiator::Them, "they still began it");
    assert_eq!(day_two.they_followed_at, Some(origin()));
    assert_eq!(day_two.i_followed_at, Some(later(1)));
}

#[test]
fn a_follower_who_leaves_has_their_departure_recorded() {
    let them = [user(1, "them")];

    let followed = only(attribute(&established(), &them, &them, origin()));
    let left = only(attribute(&[followed], &them, &[], later(5)));

    assert_eq!(left.they_unfollowed_at, Some(later(5)));
}

#[test]
fn a_returning_follower_restarts_the_window() {
    let them = [user(1, "them")];

    let followed = only(attribute(&established(), &[], &them, origin()));
    let left = only(attribute(&[followed], &them, &[], later(5)));
    let returned = only(attribute(&[left], &them, &them, later(9)));

    assert_eq!(
        returned.they_unfollowed_at, None,
        "they are following again"
    );
    assert_eq!(
        returned.they_followed_at,
        Some(later(9)),
        "the clock restarts, so a leave-and-return cannot run it down"
    );
}

#[test]
fn a_renamed_account_keeps_its_history_under_the_new_name() {
    let before = [user(7, "old-name")];
    let after = [user(7, "new-name")];

    let first = only(attribute(&established(), &[], &before, origin()));
    let second = only(attribute(&[first], &[], &after, later(2)));

    assert_eq!(
        second.login, "new-name",
        "the display name follows the account"
    );
    assert_eq!(
        second.they_followed_at,
        Some(origin()),
        "a rename is not a new relationship"
    );
}

#[test]
fn every_account_appears_once_even_when_on_both_sides() {
    let both = [user(1, "a"), user(2, "b")];

    let updated = attribute(&established(), &both, &both, origin());

    assert_eq!(updated.len(), 2, "mutuals must not be recorded twice");
}
