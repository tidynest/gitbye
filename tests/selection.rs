//! Who the unattended sweep actually picks.
//!
//! `should_sweep` decides one relationship at a time and is covered in
//! `sweeping.rs`. This covers the step around it, which turns the whole record
//! into a list of accounts to unfollow. It is the last thing that runs before
//! something irreversible happens to somebody's follow, so the cases that must
//! never be selected are pinned down here rather than trusted to the caller.

use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gitbye::model::{DAY, DEFAULT_GRACE, Initiator, Relationship};
use gitbye::sweep::select;

fn origin() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// Somebody who followed, was followed back, and left after the given time.
fn churner(id: i64, login: &str, held: Duration) -> Relationship {
    Relationship {
        user_id: id,
        login: login.to_owned(),
        initiator: Initiator::Them,
        they_followed_at: Some(origin()),
        i_followed_at: Some(origin()),
        they_unfollowed_at: Some(origin() + held),
    }
}

fn ids(users: &[gitbye::model::User]) -> Vec<i64> {
    users.iter().map(|user| user.id).collect()
}

fn logins(users: &[gitbye::model::User]) -> Vec<String> {
    users.iter().map(|user| user.login.clone()).collect()
}

fn following(ids: &[i64]) -> HashSet<i64> {
    ids.iter().copied().collect()
}

fn nobody() -> HashSet<i64> {
    HashSet::new()
}

#[test]
fn it_selects_someone_who_took_the_follow_back_and_left() {
    let record = [churner(1, "magpie", DAY * 9)];

    let chosen = select(&record, &nobody(), &following(&[1]), DEFAULT_GRACE);

    assert_eq!(ids(&chosen), vec![1]);
}

#[test]
fn it_never_selects_anyone_you_no_longer_follow() {
    // Unfollowing somebody you are not following is a wasted call at best, and
    // at worst it means the record and the live graph have diverged.
    let record = [churner(1, "magpie", DAY * 9)];

    let chosen = select(&record, &nobody(), &nobody(), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}

#[test]
fn it_never_selects_anyone_on_the_keep_list() {
    let record = [churner(1, "magpie", DAY * 9)];
    let kept: HashSet<i64> = [1].into_iter().collect();

    let chosen = select(&record, &kept, &following(&[1]), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}

#[test]
fn it_never_selects_a_follow_you_began() {
    let mut mine = churner(1, "heron", DAY * 9);
    mine.initiator = Initiator::Me;

    let chosen = select(&[mine], &nobody(), &following(&[1]), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}

#[test]
fn it_never_selects_a_relationship_that_predates_the_record() {
    let mut ancient = churner(1, "rook", DAY * 9);
    ancient.initiator = Initiator::Unknown;

    let chosen = select(&[ancient], &nobody(), &following(&[1]), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}

#[test]
fn it_leaves_alone_anyone_who_stayed_past_the_window() {
    let loyal = [churner(1, "thrush", DEFAULT_GRACE + DAY)];

    let chosen = select(&loyal, &nobody(), &following(&[1]), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}

#[test]
fn the_window_it_is_given_is_the_one_it_applies() {
    let record = [churner(1, "magpie", DAY * 30)];

    let wide = select(&record, &nobody(), &following(&[1]), DAY * 42);
    let narrow = select(&record, &nobody(), &following(&[1]), DAY * 14);

    assert_eq!(ids(&wide), vec![1]);
    assert!(narrow.is_empty());
}

#[test]
fn the_order_is_stable_and_case_insensitive() {
    // The order reaches the user in a report and, on a real run, decides who is
    // unfollowed first. Sorting by raw bytes would file every capitalised login
    // ahead of every lowercase one, which reads as arbitrary.
    let record = [
        churner(1, "zebra", DAY),
        churner(2, "Albatross", DAY),
        churner(3, "moorhen", DAY),
    ];

    let chosen = select(&record, &nobody(), &following(&[1, 2, 3]), DEFAULT_GRACE);

    assert_eq!(logins(&chosen), vec!["Albatross", "moorhen", "zebra"]);
}

#[test]
fn it_sifts_a_mixed_record_down_to_only_the_eligible() {
    let mut began_by_me = churner(2, "heron", DAY * 5);
    began_by_me.initiator = Initiator::Me;
    let mut predates = churner(4, "rook", DAY * 5);
    predates.initiator = Initiator::Unknown;

    let record = [
        churner(1, "magpie", DAY * 5),
        began_by_me,
        churner(3, "starling", DEFAULT_GRACE + DAY),
        predates,
        churner(5, "jackdaw", DAY * 5),
        churner(6, "wren", DAY * 5),
    ];
    let kept: HashSet<i64> = [6].into_iter().collect();

    let chosen = select(
        &record,
        &kept,
        &following(&[1, 2, 3, 4, 5, 6]),
        DEFAULT_GRACE,
    );

    // magpie and jackdaw only: the others are respectively yours, loyal,
    // unknowable, and protected.
    assert_eq!(logins(&chosen), vec!["jackdaw", "magpie"]);
}

#[test]
fn an_empty_record_selects_nobody() {
    let chosen = select(&[], &nobody(), &following(&[1, 2, 3]), DEFAULT_GRACE);

    assert!(chosen.is_empty());
}
