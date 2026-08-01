//! Tests for the follow-graph set difference.
//!
//! The safety of this application rests on one question: can an account on the
//! keep-list ever appear in the bucket that unfollowing acts upon? Several of the
//! cases below exist purely to answer that with no.

use gitbye::model::{User, bucket};

fn user(id: i64, login: &str) -> User {
    User {
        id,
        login: login.to_owned(),
    }
}

fn logins(users: &[User]) -> Vec<&str> {
    users.iter().map(|user| user.login.as_str()).collect()
}

#[test]
fn empty_input_yields_empty_buckets() {
    let buckets = bucket(&[], &[], &[]);

    assert!(buckets.unreciprocated.is_empty());
    assert!(buckets.keeping.is_empty());
    assert!(buckets.mutuals.is_empty());
    assert!(buckets.fans.is_empty());
}

#[test]
fn everyone_reciprocating_yields_only_mutuals() {
    let following = [user(1, "alice"), user(2, "bob")];
    let followers = [user(2, "bob"), user(1, "alice")];

    let buckets = bucket(&following, &followers, &[]);

    assert_eq!(logins(&buckets.mutuals), ["alice", "bob"]);
    assert!(buckets.unreciprocated.is_empty());
    assert!(buckets.keeping.is_empty());
    assert!(buckets.fans.is_empty());
}

#[test]
fn nobody_reciprocating_splits_into_unreciprocated_and_fans() {
    let following = [user(1, "alice")];
    let followers = [user(2, "bob")];

    let buckets = bucket(&following, &followers, &[]);

    assert_eq!(logins(&buckets.unreciprocated), ["alice"]);
    assert_eq!(logins(&buckets.fans), ["bob"]);
    assert!(buckets.mutuals.is_empty());
    assert!(buckets.keeping.is_empty());
}

#[test]
fn keep_list_shields_an_account_from_the_unfollow_bucket() {
    let following = [user(1, "alice"), user(2, "bob")];

    let buckets = bucket(&following, &[], &[2]);

    assert_eq!(logins(&buckets.keeping), ["bob"]);
    assert_eq!(logins(&buckets.unreciprocated), ["alice"]);
}

#[test]
fn keep_list_still_shields_an_account_after_it_is_renamed() {
    // The keep-list stores id 7. The account has since renamed itself, so the
    // login recorded when it was added no longer matches anything.
    let following = [user(7, "renamed-since-being-kept")];

    let buckets = bucket(&following, &[], &[7]);

    assert_eq!(logins(&buckets.keeping), ["renamed-since-being-kept"]);
    assert!(buckets.unreciprocated.is_empty());
}

#[test]
fn keep_list_entry_for_a_mutual_lies_dormant() {
    let following = [user(3, "carol")];
    let followers = [user(3, "carol")];

    let buckets = bucket(&following, &followers, &[3]);

    assert_eq!(logins(&buckets.mutuals), ["carol"]);
    assert!(buckets.keeping.is_empty());
}

#[test]
fn buckets_are_sorted_case_insensitively() {
    let following = [user(1, "Zeta"), user(2, "alpha"), user(3, "Beta")];

    let buckets = bucket(&following, &[], &[]);

    assert_eq!(logins(&buckets.unreciprocated), ["alpha", "Beta", "Zeta"]);
}

#[test]
fn every_account_is_counted_exactly_once() {
    let following = [
        user(1, "alice"),
        user(2, "bob"),
        user(3, "carol"),
        user(4, "dave"),
    ];
    let followers = [user(3, "carol"), user(4, "dave"), user(5, "erin")];

    let buckets = bucket(&following, &followers, &[2]);

    assert_eq!(
        buckets.unreciprocated.len() + buckets.keeping.len() + buckets.mutuals.len(),
        following.len(),
        "every followed account belongs to exactly one of the three following buckets"
    );
    assert_eq!(
        buckets.mutuals.len() + buckets.fans.len(),
        followers.len(),
        "every follower is either a mutual or a fan"
    );
}
