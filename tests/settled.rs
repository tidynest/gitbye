//! Why the actionable bucket is empty.
//!
//! The keep-list is subtracted from "Not following back", so an empty bucket
//! does not mean everyone reciprocates. Reporting it as though it did claims
//! something the counts displayed beside it plainly contradict.

use gitbye::model::{Settled, User, bucket};

fn user(id: i64, login: &str) -> User {
    User {
        id,
        login: login.to_owned(),
    }
}

#[test]
fn an_empty_bucket_with_nobody_kept_means_everyone_reciprocates() {
    let me_following = vec![user(1, "ada"), user(2, "grace")];
    let following_me = vec![user(1, "ada"), user(2, "grace")];

    let buckets = bucket(&me_following, &following_me, &[]);

    assert!(buckets.unreciprocated.is_empty());
    assert_eq!(buckets.settled(), Settled::Mutual);
}

#[test]
fn an_empty_bucket_with_accounts_kept_does_not_mean_everyone_reciprocates() {
    // Two accounts do not follow back. Both are kept, so nothing is actionable,
    // but claiming everyone reciprocates would be false.
    let me_following = vec![user(1, "ada"), user(2, "grace"), user(3, "linus")];
    let following_me = vec![user(1, "ada")];

    let buckets = bucket(&me_following, &following_me, &[2, 3]);

    assert!(buckets.unreciprocated.is_empty());
    assert_eq!(buckets.settled(), Settled::AllKept(2));
}

#[test]
fn a_single_kept_account_is_counted_as_one() {
    let me_following = vec![user(1, "ada"), user(2, "grace")];
    let following_me = vec![user(1, "ada")];

    let buckets = bucket(&me_following, &following_me, &[2]);

    assert_eq!(buckets.settled(), Settled::AllKept(1));
}

#[test]
fn reciprocation_outranks_the_keep_list() {
    // Keeping someone who follows back leaves them in mutuals, so they must not
    // be counted among the accounts that fail to reciprocate.
    let me_following = vec![user(1, "ada")];
    let following_me = vec![user(1, "ada")];

    let buckets = bucket(&me_following, &following_me, &[1]);

    assert!(buckets.keeping.is_empty());
    assert_eq!(buckets.settled(), Settled::Mutual);
}
