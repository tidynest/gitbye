//! GitHub REST client, plus retrieval of the token it authenticates with.
//!
//! This application deliberately stores no credential of its own. The token is
//! borrowed from the already authenticated `gh` command line tool, which holds it
//! in the operating system keyring.

use std::process::Command;

use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client as HttpClient, RequestBuilder};
use reqwest::{Method, StatusCode};

use crate::model::User;

/// Base address of the GitHub REST API.
const API: &str = "https://api.github.com";

/// Maximum accounts GitHub returns per page.
const PER_PAGE: usize = 100;

/// Upper bound on pages walked, so a misbehaving response cannot loop forever.
const MAX_PAGES: usize = 100;

/// Sent as the User-Agent, which GitHub requires on every request.
const AGENT: &str = concat!("gitbye/", env!("CARGO_PKG_VERSION"));

/// The command that grants the scope needed to follow and unfollow.
pub const SCOPE_FIX: &str = "gh auth refresh -h github.com -s user:follow";

/// Borrows the current token from the `gh` command line tool.
///
/// # Errors
///
/// Fails when `gh` is absent, not authenticated, or returns nothing. Each case
/// carries the command that resolves it.
pub fn token() -> Result<String> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .context("could not run `gh`. Install the GitHub CLI, then run `gh auth login`")?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`gh auth token` failed. Run `gh auth login` first.\n{}",
            detail.trim()
        );
    }

    let token = String::from_utf8(output.stdout)
        .context("`gh auth token` returned a token that was not valid UTF-8")?
        .trim()
        .to_owned();

    if token.is_empty() {
        bail!("`gh auth token` returned nothing. Run `gh auth login` first.");
    }

    Ok(token)
}

/// An authenticated GitHub REST client.
pub struct Github {
    http: HttpClient,
    token: String,
}

impl Github {
    /// Builds a client around an existing token.
    ///
    /// # Errors
    ///
    /// Fails if the underlying HTTP client cannot be constructed.
    pub fn new(token: String) -> Result<Self> {
        let http = HttpClient::builder()
            .user_agent(AGENT)
            .build()
            .context("could not build the HTTP client")?;
        Ok(Self { http, token })
    }

    /// Applies the authentication and versioning headers every call needs.
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{API}{path}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// Walks every page of a listing endpoint and collects the accounts.
    fn paginate(&self, path: &str) -> Result<Vec<User>> {
        let mut collected = Vec::new();

        for page in 1..=MAX_PAGES {
            let url = format!("{path}?per_page={PER_PAGE}&page={page}");
            let batch: Vec<User> = self
                .request(Method::GET, &url)
                .send()
                .with_context(|| format!("could not reach GitHub for {path}"))?
                .error_for_status()
                .with_context(|| format!("GitHub rejected the request for {path}"))?
                .json()
                .with_context(|| format!("could not decode the response for {path}"))?;

            let last_page = batch.len() < PER_PAGE;
            collected.extend(batch);

            if last_page {
                return Ok(collected);
            }
        }

        bail!(
            "{path} returned more than {} accounts, refusing to page further",
            MAX_PAGES * PER_PAGE
        )
    }

    /// Every account the authenticated user follows.
    ///
    /// # Errors
    ///
    /// Fails when GitHub is unreachable or rejects the request.
    pub fn following(&self) -> Result<Vec<User>> {
        self.paginate("/user/following")
    }

    /// Every account following the authenticated user.
    ///
    /// # Errors
    ///
    /// Fails when GitHub is unreachable or rejects the request.
    pub fn followers(&self) -> Result<Vec<User>> {
        self.paginate("/user/followers")
    }

    /// Shared body of [`Self::follow`] and [`Self::unfollow`], which differ only
    /// by HTTP method.
    fn amend_following(&self, method: Method, login: &str) -> Result<()> {
        let response = self
            .request(method, &format!("/user/following/{login}"))
            .header("Content-Length", "0")
            .send()
            .with_context(|| format!("could not reach GitHub while acting on {login}"))?;

        match response.status() {
            StatusCode::NO_CONTENT => Ok(()),
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                bail!("permission denied. The token needs the user:follow scope: {SCOPE_FIX}")
            }
            other => bail!("GitHub answered {other}"),
        }
    }

    /// Follows an account.
    ///
    /// # Errors
    ///
    /// Fails when GitHub is unreachable, or when the token lacks `user:follow`.
    pub fn follow(&self, login: &str) -> Result<()> {
        self.amend_following(Method::PUT, login)
    }

    /// Unfollows an account.
    ///
    /// # Errors
    ///
    /// Fails when GitHub is unreachable, or when the token lacks `user:follow`.
    pub fn unfollow(&self, login: &str) -> Result<()> {
        self.amend_following(Method::DELETE, login)
    }
}
