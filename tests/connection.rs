//! Tests for connection string handling.
//!
//! These parse only. Nothing here opens a connection, so the suite still runs
//! with no database present.

use gitbye::db::{DEFAULT_URL, connection_config};

#[test]
fn a_url_without_a_host_falls_back_to_the_local_socket() {
    let config = connection_config("postgresql:///gitbye").expect("valid connection string");

    assert!(
        !config.get_hosts().is_empty(),
        "a host must be filled in, or the driver rejects the string outright"
    );
    assert!(
        format!("{config:?}").contains("/run/postgresql"),
        "the fallback should be the local socket directory"
    );
}

#[test]
fn an_explicit_host_is_left_alone() {
    let config =
        connection_config("postgresql://db.example.com/gitbye").expect("valid connection string");

    let rendered = format!("{config:?}");
    assert!(
        rendered.contains("db.example.com"),
        "the given host must survive"
    );
    assert!(
        !rendered.contains("/run/postgresql"),
        "the fallback must not be added on top of a host that was supplied"
    );
}

#[test]
fn a_malformed_connection_string_is_rejected() {
    assert!(connection_config("this is not a connection string").is_err());
}

#[test]
fn the_default_names_this_application_s_own_database_over_the_local_socket() {
    // A launcher entry inherits no shell environment, so this is what runs when
    // nothing has been exported. It must not need a host.
    let config = connection_config(DEFAULT_URL).expect("the default must parse");

    assert_eq!(config.get_dbname(), Some("gitbye"));
    assert!(
        !config.get_hosts().is_empty(),
        "the socket directory must be filled in, or the driver refuses"
    );
}
