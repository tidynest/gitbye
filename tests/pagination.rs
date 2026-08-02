//! Walking the pages of a listing endpoint.
//!
//! This decides how much of the follow graph the application ever sees, and it
//! fails silently in both directions. Stop a page early and accounts vanish
//! without an error: the buckets under-report and the sweep judges against a
//! partial picture. Never stop and it walks forever. Neither shows up as a
//! crash, so both are pinned down here.
//!
//! The fetcher is supplied by the test, so none of this touches a network.

use anyhow::{Result, anyhow};
use gitbye::github::{MAX_PAGES, PER_PAGE, collect_pages};
use gitbye::model::User;

/// A page of `count` accounts, numbered so their order can be checked.
fn page(from: i64, count: usize) -> Vec<User> {
    (0..i64::try_from(count).expect("a test page fits in i64"))
        .map(|offset| {
            let id = from + offset;
            User {
                id,
                login: format!("account{id}"),
            }
        })
        .collect()
}

/// Serves the given pages in order, and records how many were asked for.
fn serving(
    pages: Vec<Vec<User>>,
) -> (
    impl FnMut(usize) -> Result<Vec<User>>,
    std::rc::Rc<std::cell::Cell<usize>>,
) {
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let counter = std::rc::Rc::clone(&asked);

    let fetch = move |page: usize| {
        counter.set(counter.get().max(page));
        Ok(pages.get(page - 1).cloned().unwrap_or_default())
    };

    (fetch, asked)
}

#[test]
fn a_single_short_page_is_the_whole_answer() {
    let (fetch, asked) = serving(vec![page(1, 7)]);

    let all = collect_pages("/user/following", fetch).expect("a short page ends the walk");

    assert_eq!(all.len(), 7);
    assert_eq!(
        asked.get(),
        1,
        "a short page must not provoke another request"
    );
}

#[test]
fn an_empty_first_page_is_an_empty_answer_not_an_error() {
    // Following nobody is an ordinary state, not a failure.
    let (fetch, asked) = serving(vec![Vec::new()]);

    let all = collect_pages("/user/following", fetch).expect("nobody is a valid answer");

    assert!(all.is_empty());
    assert_eq!(asked.get(), 1);
}

#[test]
fn a_full_page_provokes_another_request() {
    // The fault this guards: treating a full page as the end silently drops
    // everyone after the hundredth account.
    let (fetch, asked) = serving(vec![page(1, PER_PAGE), page(1000, 3)]);

    let all = collect_pages("/user/following", fetch).expect("a full page is not proof of the end");

    assert_eq!(all.len(), PER_PAGE + 3);
    assert_eq!(asked.get(), 2);
}

#[test]
fn a_run_of_full_pages_is_walked_to_the_end() {
    let (fetch, asked) = serving(vec![
        page(1, PER_PAGE),
        page(1000, PER_PAGE),
        page(2000, PER_PAGE),
        page(3000, 12),
    ]);

    let all = collect_pages("/user/followers", fetch).expect("walks until a short page");

    assert_eq!(all.len(), PER_PAGE * 3 + 12);
    assert_eq!(asked.get(), 4);
}

#[test]
fn a_final_page_that_is_exactly_empty_still_terminates() {
    // GitHub answers a full last page followed by an empty one. That empty page
    // is shorter than the maximum, so it ends the walk.
    let (fetch, asked) = serving(vec![page(1, PER_PAGE), Vec::new()]);

    let all = collect_pages("/user/following", fetch).expect("an empty page ends the walk");

    assert_eq!(all.len(), PER_PAGE);
    assert_eq!(asked.get(), 2);
}

#[test]
fn accounts_keep_the_order_the_pages_arrived_in() {
    let (fetch, _) = serving(vec![page(1, PER_PAGE), page(500, 2)]);

    let all = collect_pages("/user/following", fetch).expect("walks both pages");

    assert_eq!(all.first().map(|user| user.id), Some(1));
    assert_eq!(
        all[PER_PAGE - 1].id,
        i64::try_from(PER_PAGE).expect("the page size fits in i64")
    );
    assert_eq!(all.last().map(|user| user.id), Some(501));
}

#[test]
fn an_endpoint_that_never_shortens_is_refused_rather_than_walked_forever() {
    let mut asked = 0;

    let outcome = collect_pages("/user/following", |_| {
        asked += 1;
        Ok(page(1, PER_PAGE))
    });

    let complaint = outcome.expect_err("a listing that never ends must be refused");
    assert!(
        complaint.to_string().contains("refusing to page further"),
        "{complaint}"
    );
    assert_eq!(
        asked, MAX_PAGES,
        "it must stop at the bound, not before or after"
    );
}

#[test]
fn a_failure_on_any_page_stops_the_walk_and_is_reported() {
    // Returning what was collected so far would look like a complete graph, and
    // a partial graph is exactly what must never be acted on.
    let mut asked = 0;

    let outcome = collect_pages("/user/following", |page_number| {
        asked += 1;
        match page_number {
            1 => Ok(page(1, PER_PAGE)),
            _ => Err(anyhow!("could not reach GitHub")),
        }
    });

    assert!(
        outcome.is_err(),
        "a failed page must not be silently dropped"
    );
    assert_eq!(asked, 2, "it must stop at the failure rather than carry on");
}
