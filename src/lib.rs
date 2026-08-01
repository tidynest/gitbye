//! Compare the GitHub accounts you follow against the accounts following you,
//! then act on the difference.
//!
//! The crate is a library with a thin binary on top, because integration tests
//! live outside the code under test and can only reach a public library API.

pub mod app;
pub mod db;
pub mod github;
pub mod model;
mod theme;
mod ui;
