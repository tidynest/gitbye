//! Keeps the source-of-truth document honest.
//!
//! A document describing where the authoritative data lives is worth having only
//! for as long as it is true. Prose rots silently, so this reads the schema out
//! of the source and fails if a table exists that the document does not describe.
//! Adding a table therefore requires saying what it is authoritative for, in the
//! same commit.

const SCHEMA_SOURCE: &str = include_str!("../src/db.rs");
const DOCUMENT: &str = include_str!("../docs/sources-of-truth.md");

/// Every table name the schema creates.
fn tables() -> Vec<String> {
    SCHEMA_SOURCE
        .lines()
        .filter_map(|line| line.trim().strip_prefix("CREATE TABLE IF NOT EXISTS "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_schema_creates_the_tables_we_think_it_does() {
    // A guard on the guard: if the extraction silently stopped matching, every
    // other test here would pass by describing nothing.
    let found = tables();

    assert!(
        found.len() >= 5,
        "expected to find the schema in src/db.rs, found {found:?}"
    );
}

#[test]
fn every_table_is_described_in_the_document() {
    for table in tables() {
        assert!(
            DOCUMENT.contains(&format!("`{table}`")),
            "table `{table}` exists in the schema but is not described in \
             docs/sources-of-truth.md. Say what it is authoritative for."
        );
    }
}

#[test]
fn the_document_describes_no_table_that_was_removed() {
    // The opposite drift: a table is dropped and its section lingers, promising
    // an authority that no longer exists.
    let found = tables();

    for line in DOCUMENT.lines() {
        let Some(heading) = line.strip_prefix("### ") else {
            continue;
        };
        for name in heading.split('`').skip(1).step_by(2) {
            assert!(
                found.iter().any(|table| table == name),
                "docs/sources-of-truth.md describes `{name}`, which the schema \
                 no longer creates"
            );
        }
    }
}

#[test]
fn the_environment_variable_is_named_correctly() {
    // The document tells people which variable overrides the default. If that
    // name ever changes, the instruction becomes a wrong answer rather than a
    // missing one, which is worse.
    assert!(DOCUMENT.contains(gitbye::db::URL_VAR));
    assert!(DOCUMENT.contains(gitbye::db::DEFAULT_URL));
}
