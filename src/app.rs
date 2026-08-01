//! Application state, background job dispatch, and the frame loop.
//!
//! Every slow operation runs on a worker thread and reports back over a channel.
//! The interface thread owns all state and never blocks, so there is no mutex
//! guarding application state and no lock ordering to reason about.

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context as _, Result, anyhow};
use eframe::egui::{Color32, Context};

use crate::db::Store;
use crate::github::Github;
use crate::model::{Buckets, Msg, User, bucket};
use crate::{theme, ui};

/// Which bucket is on screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Unreciprocated,
    Keeping,
    Mutuals,
    Fans,
}

impl Tab {
    /// Tab order, left to right.
    pub(crate) const ALL: [Self; 4] = [
        Self::Unreciprocated,
        Self::Keeping,
        Self::Mutuals,
        Self::Fans,
    ];

    /// Heading shown on the tab.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Unreciprocated => "Not following back",
            Self::Keeping => "Keeping",
            Self::Mutuals => "Mutuals",
            Self::Fans => "Fans",
        }
    }

    /// Semantic colour carried by rows in this bucket.
    pub(crate) fn colour(self) -> Color32 {
        match self {
            Self::Unreciprocated => theme::UNRECIPROCATED,
            Self::Keeping => theme::PROTECTED,
            Self::Mutuals => theme::RECIPROCATED,
            Self::Fans => theme::INFORMATIONAL,
        }
    }

    /// Whether rows here can be ticked. Mutuals are the one read-only bucket.
    pub(crate) fn selectable(self) -> bool {
        !matches!(self, Self::Mutuals)
    }
}

/// Which write a batch performs. Both directions share one runner, so the
/// failure policy exists in exactly one place.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Follow,
    Unfollow,
}

impl Action {
    /// Performs the write for a single account.
    fn apply(self, github: &Github, login: &str) -> Result<()> {
        match self {
            Self::Follow => github.follow(login),
            Self::Unfollow => github.unfollow(login),
        }
    }

    /// Label used on the confirming button and in the summary.
    pub(crate) fn verb(self) -> &'static str {
        match self {
            Self::Follow => "Follow",
            Self::Unfollow => "Unfollow",
        }
    }
}

/// A message across the top of the window.
pub(crate) struct Banner {
    pub(crate) message: String,
    pub(crate) is_error: bool,
}

/// A pending write, awaiting confirmation.
pub(crate) struct Confirm {
    pub(crate) action: Action,
    pub(crate) targets: Vec<User>,
}

/// How far through a batch the worker has reached.
pub(crate) struct Progress {
    pub(crate) done: usize,
    pub(crate) total: usize,
    pub(crate) login: String,
}

/// Sends a message to the interface thread and wakes it, so progress appears as
/// it happens rather than at the next unrelated input event.
pub(crate) struct Reporter {
    tx: Sender<Msg>,
    ctx: Context,
}

impl Reporter {
    /// Delivers one message and requests a repaint.
    fn send(&self, message: Msg) {
        self.tx.send(message).ok();
        self.ctx.request_repaint();
    }
}

/// Borrows the store for one operation.
///
/// Concentrating the lock and the availability check here means the callers
/// never handle a poisoned lock or an absent store themselves.
fn with_store<T>(
    store: &Mutex<Option<Store>>,
    job: impl FnOnce(&mut Store) -> Result<T>,
) -> Result<T> {
    let mut guard = store
        .lock()
        .map_err(|_| anyhow!("the store lock was poisoned by an earlier failure"))?;
    let store = guard.as_mut().context("the keep-list is unavailable")?;
    job(store)
}

/// Fetches both sides of the follow graph, records the sync, and reads the
/// keep-list.
fn sync_job(github: &Github, store: &Mutex<Option<Store>>) -> Msg {
    let following = match github.following() {
        Ok(users) => users,
        Err(error) => return Msg::Error(format!("{error:#}")),
    };
    let followers = match github.followers() {
        Ok(users) => users,
        Err(error) => return Msg::Error(format!("{error:#}")),
    };

    // A store failure is not fatal. The lists still display, and `None` keeps
    // unfollowing disabled for as long as the keep-list cannot be trusted.
    let keep = with_store(store, |store| {
        store.record_sync(&following, &followers)?;
        store.keep_list()
    })
    .ok();

    Msg::Synced {
        following,
        followers,
        keep,
    }
}

/// Runs a whole batch, continuing past failures and reporting them together.
///
/// A failing account never aborts the run. Stopping early would leave the batch
/// half applied with no resume point, whereas finishing and reporting produces
/// one readable summary even when a token expires midway.
fn run_batch(github: &Github, action: Action, targets: &[User], reporter: &Reporter) -> Msg {
    let total = targets.len();
    let mut ok = 0;
    let mut failed = Vec::new();

    for (index, user) in targets.iter().enumerate() {
        match action.apply(github, &user.login) {
            Ok(()) => ok += 1,
            Err(error) => failed.push((user.login.clone(), format!("{error:#}"))),
        }
        reporter.send(Msg::Progress {
            done: index + 1,
            total,
            login: user.login.clone(),
        });
    }

    Msg::Finished { ok, failed }
}

/// The whole application.
pub struct GoodbyeApp {
    pub(crate) tab: Tab,
    pub(crate) buckets: Buckets,
    pub(crate) selected: HashSet<i64>,
    pub(crate) banner: Option<Banner>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) progress: Option<Progress>,
    pub(crate) busy: bool,
    pub(crate) keep_ready: bool,
    following: Vec<User>,
    followers: Vec<User>,
    keep: Vec<i64>,
    github: Arc<Github>,
    store: Arc<Mutex<Option<Store>>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl GoodbyeApp {
    /// Builds the application, applies the palette, and starts the first sync.
    ///
    /// A database that cannot be reached is reported and survived. Only a
    /// missing GitHub token is fatal, and that is handled before this point.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, github: Github) -> Self {
        theme::apply(&cc.egui_ctx);
        let (tx, rx) = channel();

        let (store, banner) = match Store::connect() {
            Ok(store) => (Some(store), None),
            Err(error) => (
                None,
                Some(Banner {
                    message: format!("Keep-list unavailable: {error:#}. Unfollowing is disabled."),
                    is_error: true,
                }),
            ),
        };

        let mut app = Self {
            tab: Tab::Unreciprocated,
            buckets: Buckets::default(),
            selected: HashSet::new(),
            banner,
            confirm: None,
            progress: None,
            busy: false,
            keep_ready: false,
            following: Vec::new(),
            followers: Vec::new(),
            keep: Vec::new(),
            github: Arc::new(github),
            store: Arc::new(Mutex::new(store)),
            tx,
            rx,
        };

        app.sync(&cc.egui_ctx);
        app
    }

    /// Runs a job on a worker thread and reports its outcome back.
    fn spawn<F>(&mut self, ctx: &Context, job: F)
    where
        F: FnOnce(&Reporter) -> Msg + Send + 'static,
    {
        self.busy = true;
        self.progress = None;

        let reporter = Reporter {
            tx: self.tx.clone(),
            ctx: ctx.clone(),
        };
        thread::spawn(move || {
            let outcome = job(&reporter);
            reporter.send(outcome);
        });
    }

    /// Refreshes both sides of the follow graph.
    pub(crate) fn sync(&mut self, ctx: &Context) {
        let github = Arc::clone(&self.github);
        let store = Arc::clone(&self.store);
        self.spawn(ctx, move |_| sync_job(&github, &store));
    }

    /// Adds the current selection to the keep-list.
    pub(crate) fn keep_selected(&mut self, ctx: &Context) {
        let targets = self.selection();
        let store = Arc::clone(&self.store);
        self.spawn(ctx, move |_| {
            let outcome = with_store(&store, |store| {
                store.keep(&targets)?;
                store.keep_list()
            });
            match outcome {
                Ok(keep) => Msg::KeepList(keep),
                Err(error) => Msg::Error(format!("{error:#}")),
            }
        });
    }

    /// Removes the current selection from the keep-list.
    pub(crate) fn unkeep_selected(&mut self, ctx: &Context) {
        let ids: Vec<i64> = self.selection().iter().map(|user| user.id).collect();
        let store = Arc::clone(&self.store);
        self.spawn(ctx, move |_| {
            let outcome = with_store(&store, |store| {
                store.unkeep(&ids)?;
                store.keep_list()
            });
            match outcome {
                Ok(keep) => Msg::KeepList(keep),
                Err(error) => Msg::Error(format!("{error:#}")),
            }
        });
    }

    /// Raises the confirmation dialogue for a write.
    pub(crate) fn ask(&mut self, action: Action) {
        self.confirm = Some(Confirm {
            action,
            targets: self.selection(),
        });
    }

    /// Runs the write the user confirmed.
    pub(crate) fn run_confirmed(&mut self, ctx: &Context) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        let github = Arc::clone(&self.github);
        self.spawn(ctx, move |reporter| {
            run_batch(&github, confirm.action, &confirm.targets, reporter)
        });
    }

    /// Rows of the tab currently on screen.
    pub(crate) fn rows(&self) -> &[User] {
        match self.tab {
            Tab::Unreciprocated => &self.buckets.unreciprocated,
            Tab::Keeping => &self.buckets.keeping,
            Tab::Mutuals => &self.buckets.mutuals,
            Tab::Fans => &self.buckets.fans,
        }
    }

    /// How many rows each tab holds, for the tab bar counts.
    pub(crate) fn count(&self, tab: Tab) -> usize {
        match tab {
            Tab::Unreciprocated => self.buckets.unreciprocated.len(),
            Tab::Keeping => self.buckets.keeping.len(),
            Tab::Mutuals => self.buckets.mutuals.len(),
            Tab::Fans => self.buckets.fans.len(),
        }
    }

    /// The ticked rows of the current tab.
    pub(crate) fn selection(&self) -> Vec<User> {
        self.rows()
            .iter()
            .filter(|user| self.selected.contains(&user.id))
            .cloned()
            .collect()
    }

    /// Whether a write may be started right now.
    pub(crate) fn can_write(&self, action: Action) -> bool {
        // Following is never gated on the keep-list, which only ever guards
        // against unfollowing.
        let guarded = action == Action::Unfollow && !self.keep_ready;
        !self.busy && !guarded && !self.selected.is_empty()
    }

    /// Recomputes the buckets and drops any selection that no longer applies.
    fn recompute(&mut self) {
        self.buckets = bucket(&self.following, &self.followers, &self.keep);
        self.selected.clear();
    }

    /// Turns a finished batch into a banner.
    fn summarise(&mut self, action_done: usize, failed: &[(String, String)]) {
        if failed.is_empty() {
            let message = format!("Done. {action_done} succeeded.");
            self.banner = Some(Banner {
                message,
                is_error: false,
            });
            return;
        }

        let detail = failed
            .iter()
            .map(|(login, reason)| format!("{login}: {reason}"))
            .collect::<Vec<_>>()
            .join("; ");
        let message = format!("{action_done} succeeded, {} failed. {detail}", failed.len());
        self.banner = Some(Banner {
            message,
            is_error: true,
        });
    }

    /// Applies every message the workers have queued since the last frame.
    fn drain(&mut self, ctx: &Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Msg::Synced {
                    following,
                    followers,
                    keep,
                } => {
                    self.following = following;
                    self.followers = followers;
                    self.keep_ready = keep.is_some();
                    self.keep = keep.unwrap_or_default();
                    self.recompute();
                    self.busy = false;
                    self.progress = None;
                }
                Msg::KeepList(keep) => {
                    self.keep = keep;
                    self.keep_ready = true;
                    self.recompute();
                    self.busy = false;
                }
                Msg::Progress { done, total, login } => {
                    self.progress = Some(Progress { done, total, login });
                }
                Msg::Finished { ok, failed } => {
                    self.summarise(ok, &failed);
                    self.progress = None;
                    // The graph has changed, so the buckets on screen are stale.
                    self.sync(ctx);
                }
                Msg::Error(message) => {
                    self.banner = Some(Banner {
                        message,
                        is_error: true,
                    });
                    self.busy = false;
                    self.progress = None;
                }
            }
        }
    }
}

impl eframe::App for GoodbyeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        ui::draw(self, ctx);
    }
}
