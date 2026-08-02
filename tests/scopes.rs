//! Reading the scope list GitHub reports for a token.
//!
//! Getting this wrong in one direction costs a clear error at the point of use.
//! Getting it wrong in the other silently withdraws unfollowing from a token
//! that was entitled to it, so the absent and empty cases are pinned down here.

use gitbye::github::grants_follow;

#[test]
fn a_list_containing_the_scope_grants_it() {
    assert!(grants_follow(
        "admin:org, project, repo, user:follow, workflow"
    ));
}

#[test]
fn a_list_without_the_scope_withholds_it() {
    assert!(!grants_follow(
        "admin:public_key, gist, project, read:org, repo"
    ));
}

#[test]
fn the_scope_is_found_wherever_it_sits() {
    assert!(grants_follow("user:follow"));
    assert!(grants_follow("user:follow, repo"));
    assert!(grants_follow("repo, user:follow"));
}

#[test]
fn an_empty_list_grants_it() {
    // A fine-grained token reports no scopes at all. That says nothing about
    // what it may do, so it must not be read as a refusal.
    assert!(grants_follow(""));
    assert!(grants_follow("   "));
}

#[test]
fn a_broader_scope_is_not_mistaken_for_this_one() {
    // Substring matching would accept both of these. Neither grants following.
    assert!(!grants_follow("user"));
    assert!(!grants_follow("user:email, read:user"));
}

#[test]
fn surrounding_space_does_not_hide_the_scope() {
    assert!(grants_follow("repo,user:follow"));
    assert!(grants_follow("  repo ,  user:follow  "));
}
