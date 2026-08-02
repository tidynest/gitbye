//! Reading, describing and applying the sweep window.
//!
//! The window decides whether a follow is withdrawn, so a misread figure is not
//! a cosmetic fault. The bounds matter for the same reason: nothing would sweep
//! anyone who ever left, and a year quietly sweeps nobody.

use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gitbye::model::{
    DAY, DEFAULT_GRACE, Initiator, MAX_GRACE, MIN_GRACE, Relationship, Unit, describe_grace,
    parse_grace, should_sweep, split_grace,
};

fn origin() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// Someone who followed, was followed back, and left after the given time.
fn left_after(held: Duration) -> Relationship {
    Relationship {
        user_id: 1,
        login: "transient".to_owned(),
        initiator: Initiator::Them,
        they_followed_at: Some(origin()),
        i_followed_at: Some(origin()),
        they_unfollowed_at: Some(origin() + held),
    }
}

fn nobody_kept() -> HashSet<i64> {
    HashSet::new()
}

#[test]
fn weeks_and_days_both_read() {
    assert_eq!(parse_grace("6w"), Ok(DAY * 42));
    assert_eq!(parse_grace("45d"), Ok(DAY * 45));
}

#[test]
fn a_bare_number_is_days() {
    assert_eq!(parse_grace("30"), Ok(DAY * 30));
}

#[test]
fn spacing_and_capitals_do_not_matter() {
    assert_eq!(parse_grace("  6W  "), Ok(DAY * 42));
    assert_eq!(parse_grace("10W"), Ok(DEFAULT_GRACE));
}

#[test]
fn nonsense_is_refused_with_a_readable_reason() {
    let complaint = parse_grace("soon").unwrap_err();
    assert!(complaint.contains("not a window"), "{complaint}");
    assert!(parse_grace("").is_err());
    assert!(parse_grace("-4w").is_err());
}

#[test]
fn the_bounds_are_refused_rather_than_clamped() {
    // Nothing would withdraw a follow from anyone who ever left, and a window
    // nothing can fall inside disables the sweep without saying so.
    assert!(parse_grace("0d").is_err());
    assert!(parse_grace("6y").is_err());

    assert_eq!(parse_grace("1d"), Ok(MIN_GRACE));
    assert_eq!(parse_grace("5y"), Ok(MAX_GRACE));
}

#[test]
fn every_unit_can_be_written() {
    assert_eq!(parse_grace("10d"), Ok(DAY * 10));
    assert_eq!(parse_grace("10w"), Ok(DAY * 70));
    assert_eq!(parse_grace("3m"), Ok(DAY * 90));
    assert_eq!(parse_grace("1y"), Ok(DAY * 365));
}

#[test]
fn a_window_is_split_into_the_largest_unit_that_divides_it() {
    assert_eq!(split_grace(DAY * 90), (3, Unit::Months));
    assert_eq!(split_grace(DAY * 365), (1, Unit::Years));
    assert_eq!(split_grace(DAY * 70), (10, Unit::Weeks));
    // Not roundable, so it stays in days rather than misstating the rule.
    assert_eq!(split_grace(DAY * 71), (71, Unit::Days));
}

#[test]
fn units_name_themselves_singular_or_plural() {
    assert_eq!(Unit::Days.name(1), "day");
    assert_eq!(Unit::Weeks.name(2), "weeks");
    assert_eq!(Unit::Months.name(1), "month");
    assert_eq!(Unit::Years.name(5), "years");
}

#[test]
fn a_window_is_named_in_the_unit_it_was_meant_in() {
    assert_eq!(describe_grace(DAY * 70), "10 weeks");
    assert_eq!(describe_grace(DAY * 45), "45 days");
    assert_eq!(describe_grace(DAY * 90), "3 months");
    assert_eq!(describe_grace(DAY * 365), "1 year");
    assert_eq!(describe_grace(DAY * 7), "1 week");
    assert_eq!(describe_grace(DAY), "1 day");
    // Not "0 weeks", which reads as a rounding rather than as none at all.
    assert_eq!(describe_grace(Duration::ZERO), "0 days");
}

#[test]
fn shortening_the_window_spares_someone_it_would_have_swept() {
    let brief = left_after(DAY * 30);

    assert!(should_sweep(&brief, &nobody_kept(), DAY * 42));
    assert!(!should_sweep(&brief, &nobody_kept(), DAY * 14));
}

#[test]
fn lengthening_the_window_selects_someone_it_would_have_spared() {
    let lingering = left_after(DAY * 100);

    assert!(!should_sweep(&lingering, &nobody_kept(), DEFAULT_GRACE));
    assert!(should_sweep(&lingering, &nobody_kept(), DAY * 200));
}

#[test]
fn the_keep_list_still_outranks_any_window() {
    let brief = left_after(DAY);
    let kept: HashSet<i64> = [1].into_iter().collect();

    assert!(!should_sweep(&brief, &kept, MAX_GRACE));
}
