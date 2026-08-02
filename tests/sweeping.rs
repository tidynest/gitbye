//! Tests for the automated sweep rule.
//!
//! This is the only place in the application where an account can be unfollowed
//! without a person present, so every clause of the rule is pinned down here,
//! including the ones that must refuse.

use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use gitbye::model::{DEFAULT_GRACE, Initiator, Relationship, should_sweep};

/// A fixed origin, so no test depends on the wall clock.
fn origin() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn after(days: u64) -> SystemTime {
    origin() + Duration::from_secs(days * 24 * 60 * 60)
}

/// The ordinary case: they followed, you returned it, they left inside the window.
fn farmer() -> Relationship {
    Relationship {
        user_id: 1,
        login: "farmer".to_owned(),
        initiator: Initiator::Them,
        they_followed_at: Some(origin()),
        i_followed_at: Some(after(1)),
        they_unfollowed_at: Some(after(14)),
    }
}

fn nobody_kept() -> HashSet<i64> {
    HashSet::new()
}

#[test]
fn a_follower_who_leaves_inside_the_window_is_swept() {
    assert!(should_sweep(&farmer(), &nobody_kept(), DEFAULT_GRACE));
}

#[test]
fn a_follower_who_stays_past_the_window_is_kept() {
    let mut patient = farmer();
    // One second past ten weeks.
    patient.they_unfollowed_at = Some(origin() + DEFAULT_GRACE + Duration::from_secs(1));

    assert!(
        !should_sweep(&patient, &nobody_kept(), DEFAULT_GRACE),
        "staying the full window earns the follow, even if they leave later"
    );
}

#[test]
fn the_boundary_itself_is_kept() {
    let mut exact = farmer();
    exact.they_unfollowed_at = Some(origin() + DEFAULT_GRACE);

    assert!(
        !should_sweep(&exact, &nobody_kept(), DEFAULT_GRACE),
        "ten weeks exactly counts as having served the window"
    );
}

#[test]
fn a_follower_who_is_still_following_is_never_swept() {
    let mut loyal = farmer();
    loyal.they_unfollowed_at = None;

    assert!(!should_sweep(&loyal, &nobody_kept(), DEFAULT_GRACE));
}

#[test]
fn an_account_you_followed_first_is_never_swept() {
    let mut mine = farmer();
    mine.initiator = Initiator::Me;

    assert!(
        !should_sweep(&mine, &nobody_kept(), DEFAULT_GRACE),
        "a follow you began is your decision and only you may undo it"
    );
}

#[test]
fn a_relationship_predating_the_application_is_never_swept() {
    let mut ancient = farmer();
    ancient.initiator = Initiator::Unknown;

    assert!(
        !should_sweep(&ancient, &nobody_kept(), DEFAULT_GRACE),
        "nothing can be claimed about a relationship that was never observed"
    );
}

#[test]
fn a_follow_you_never_returned_is_never_swept() {
    let mut unreturned = farmer();
    unreturned.i_followed_at = None;

    assert!(
        !should_sweep(&unreturned, &nobody_kept(), DEFAULT_GRACE),
        "there is nothing to undo if the follow was never returned"
    );
}

#[test]
fn the_keep_list_overrides_the_rule() {
    let kept: HashSet<i64> = [1].into_iter().collect();

    assert!(
        !should_sweep(&farmer(), &kept, DEFAULT_GRACE),
        "the keep-list outranks automation, as it outranks everything else"
    );
}

#[test]
fn a_missing_start_time_refuses_rather_than_guesses() {
    let mut undated = farmer();
    undated.they_followed_at = None;

    assert!(
        !should_sweep(&undated, &nobody_kept(), DEFAULT_GRACE),
        "without a start there is no window to measure, so the answer is no"
    );
}

#[test]
fn clocks_running_backwards_refuse_rather_than_panic() {
    let mut impossible = farmer();
    impossible.they_followed_at = Some(after(30));
    impossible.they_unfollowed_at = Some(after(2));

    assert!(
        !should_sweep(&impossible, &nobody_kept(), DEFAULT_GRACE),
        "an unfollow before the follow is corrupt data, not a short window"
    );
}
