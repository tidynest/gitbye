//! Tests for turning the relationship record into a list of changes.
//!
//! The trap here is presenting a first-sync timestamp as though something
//! happened at that moment. It did not: the application merely looked.

use std::time::{Duration, SystemTime};

use gitbye::model::{Event, EventKind, Initiator, Relationship, recent_events};

fn origin() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn later(days: u64) -> SystemTime {
    origin() + Duration::from_secs(days * 24 * 60 * 60)
}

fn watched(login: &str) -> Relationship {
    Relationship {
        user_id: 1,
        login: login.to_owned(),
        initiator: Initiator::Them,
        they_followed_at: Some(origin()),
        i_followed_at: Some(later(1)),
        they_unfollowed_at: None,
    }
}

fn kinds(events: &[Event]) -> Vec<EventKind> {
    events.iter().map(|event| event.kind).collect()
}

#[test]
fn a_watched_relationship_yields_an_event_per_recorded_moment() {
    let events = recent_events(&[watched("them")], 10);

    assert_eq!(events.len(), 2);
    // Newest first: you followed back on day one, they followed on day zero.
    assert_eq!(
        kinds(&events),
        [EventKind::YouFollowed, EventKind::FollowedYou]
    );
}

#[test]
fn events_are_newest_first() {
    let mut leaver = watched("leaver");
    leaver.they_unfollowed_at = Some(later(9));

    let events = recent_events(&[leaver], 10);

    assert_eq!(events[0].kind, EventKind::UnfollowedYou);
    assert_eq!(events[0].at, later(9));
}

#[test]
fn an_unobserved_beginning_contributes_no_start() {
    let mut ancient = watched("old-friend");
    ancient.initiator = Initiator::Unknown;

    let events = recent_events(&[ancient], 10);

    assert!(
        events.is_empty(),
        "a first-sync timestamp records when we looked, not when anything happened"
    );
}

#[test]
fn a_departure_counts_even_when_the_beginning_was_never_seen() {
    let mut ancient = watched("old-friend");
    ancient.initiator = Initiator::Unknown;
    ancient.they_unfollowed_at = Some(later(4));

    let events = recent_events(&[ancient], 10);

    assert_eq!(
        kinds(&events),
        [EventKind::UnfollowedYou],
        "leaving happened between two syncs, so it was genuinely observed"
    );
}

#[test]
fn the_limit_keeps_the_newest() {
    let mut first = watched("a");
    first.they_followed_at = Some(origin());
    first.i_followed_at = None;

    let mut second = watched("b");
    second.they_followed_at = Some(later(5));
    second.i_followed_at = None;

    let events = recent_events(&[first, second], 1);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].login, "b");
}

#[test]
fn an_empty_record_yields_nothing() {
    assert!(recent_events(&[], 10).is_empty());
}
