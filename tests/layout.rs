//! Tests for the responsive column maths.
//!
//! The grid is the reason the window stopped wasting two thirds of its width,
//! so the rule that decides how many columns fit is worth pinning down.

use gitbye::widgets::{MAX_COLUMNS, MIN_COLUMN, column_count};

/// Matches the gap egui leaves between items by default.
const GAP: f32 = 8.0;

#[test]
fn a_narrow_window_keeps_one_column() {
    assert_eq!(column_count(MIN_COLUMN, GAP), 1);
    assert_eq!(column_count(MIN_COLUMN * 2.0 - 1.0, GAP), 1);
}

#[test]
fn a_second_column_appears_only_once_it_fits_whole() {
    // One pixel short of two columns plus the gap between them.
    let almost = MIN_COLUMN * 2.0 + GAP - 1.0;
    assert_eq!(column_count(almost, GAP), 1);
    assert_eq!(column_count(almost + 1.0, GAP), 2);
}

#[test]
fn columns_keep_arriving_as_the_window_widens() {
    assert_eq!(column_count(MIN_COLUMN * 3.0 + GAP * 2.0, GAP), 3);
    assert_eq!(column_count(MIN_COLUMN * 4.0 + GAP * 3.0, GAP), 4);
}

#[test]
fn a_very_wide_window_stops_at_the_cap() {
    assert_eq!(column_count(10_000.0, GAP), MAX_COLUMNS);
}

#[test]
fn an_impossibly_small_window_still_yields_one_column() {
    // Never zero: a zero-column grid would divide by nothing and show nothing.
    assert_eq!(column_count(0.0, GAP), 1);
    assert_eq!(column_count(-50.0, GAP), 1);
}
