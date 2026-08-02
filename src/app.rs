//! Application state, background job dispatch, and the frame loop.
//!
//! Every slow operation runs on a worker thread and reports back over a channel.
//! The interface thread owns all state and never blocks, so there is no mutex
//! guarding application state and no lock ordering to reason about.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context as _, Result, anyhow};
use eframe::egui::{Color32, Context};

use crate::db::Store;
use crate::github::Github;
use crate::model::{
    Buckets, Event, Initiator, Msg, Recorded, Snapshot, User, attribute, bucket, recent_events,
};
use crate::{theme, ui};

/// Which of the two things the window is showing.
///
/// The accounts and their history answer different questions, so they are
/// different views rather than a fifth bucket.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Accounts,
    History,
}

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
            Self::Unreciprocated => theme::SEVER,
            Self::Keeping => theme::SHIELD,
            Self::Mutuals => theme::BOND,
            Self::Fans => theme::INBOUND,
        }
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

    /// The action in the past tense, so the option card that started it and the
    /// toast that reports it read as one sentence rather than two vocabularies.
    pub(crate) fn past(self) -> &'static str {
        match self {
            Self::Follow => "Followed",
            Self::Unfollow => "Unfollowed",
        }
    }
}

/// How many recent changes the history view lists.
const EVENT_LIMIT: usize = 40;

/// Renders a count with its noun, pluralised.
fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A persistent condition shown across the top of the window.
///
/// Distinct from a [`Toast`]: a banner describes a state that is still true, so
/// it stays until the state changes. A toast reports an event that has already
/// finished, so it leaves on its own.
pub(crate) struct Banner {
    pub(crate) message: String,
    pub(crate) is_error: bool,
}

/// How an event went, which decides the colour a toast is drawn in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Good,
    Bad,
}

/// How long a plain report of success stays.
const TOAST_LIFE: Duration = Duration::from_secs(4);

/// How long one carrying an offer of undo stays. Longer, because it asks for a
/// decision: reading time, then deciding time, then reaching the button.
const TOAST_LIFE_WITH_UNDO: Duration = Duration::from_secs(8);

/// How long a failure stays. Much longer, because nobody chose to see it and it
/// may be the only account of what went wrong.
const TOAST_LIFE_FAILED: Duration = Duration::from_secs(20);

/// A transient report of something that has finished.
pub(crate) struct Toast {
    pub(crate) message: String,
    pub(crate) tone: Tone,
    pub(crate) born: Instant,
    /// Accounts this toast offers to restore. Present only after unfollowing.
    pub(crate) undo: Option<Vec<User>>,
}

impl Toast {
    /// Total time on screen, including the fades at each end.
    pub(crate) fn life(&self) -> Duration {
        match (self.tone, self.undo.is_some()) {
            (Tone::Bad, _) => TOAST_LIFE_FAILED,
            (Tone::Good, true) => TOAST_LIFE_WITH_UNDO,
            (Tone::Good, false) => TOAST_LIFE,
        }
    }

    /// Whether it has outlived its welcome.
    pub(crate) fn expired(&self) -> bool {
        self.born.elapsed() >= self.life()
    }
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

    // Connected on first use, from a worker thread. Connecting during startup
    // would block the interface before its first frame, and an unreachable
    // server would then look exactly like a hung application. Retrying here
    // also means a database that comes back up is picked up on the next sync
    // rather than needing a restart.
    if guard.is_none() {
        *guard = Some(Store::connect()?);
    }
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
    let recorded = match with_store(store, |store| {
        store.record_sync(&following, &followers)?;
        // Attribution has to happen on every sync, whoever started it, because
        // a relationship that forms between two syncs is only ever visible as
        // the difference between them.
        let known = store.relationships()?;
        let updated = attribute(&known, &following, &followers, SystemTime::now());
        store.save_relationships(&updated)?;

        Ok(Recorded {
            keep: store.keep_list()?,
            origins: updated
                .iter()
                .map(|entry| (entry.user_id, entry.initiator))
                .collect(),
            history: store.history()?,
            events: recent_events(&updated, EVENT_LIMIT),
        })
    }) {
        Ok(recorded) => Ok(recorded),
        Err(error) => Err(format!("{error:#}")),
    };

    Msg::Synced {
        following,
        followers,
        recorded,
    }
}

/// Runs a whole batch, continuing past failures and reporting them together.
///
/// A failing account never aborts the run. Stopping early would leave the batch
/// half applied with no resume point, whereas finishing and reporting produces
/// one readable summary even when a token expires midway.
fn run_batch(github: &Github, action: Action, targets: &[User], reporter: &Reporter) -> Msg {
    let total = targets.len();
    let mut done = Vec::new();
    let mut failed = Vec::new();

    for (index, user) in targets.iter().enumerate() {
        match action.apply(github, &user.login) {
            Ok(()) => done.push(user.clone()),
            Err(error) => failed.push((user.login.clone(), format!("{error:#}"))),
        }
        reporter.send(Msg::Progress {
            done: index + 1,
            total,
            login: user.login.clone(),
        });
    }

    Msg::Finished { done, failed }
}

/// The whole application.
pub struct GitbyeApp {
    pub(crate) tab: Tab,
    pub(crate) buckets: Buckets,
    pub(crate) selected: HashSet<i64>,
    pub(crate) banner: Option<Banner>,
    pub(crate) progress: Option<Progress>,
    pub(crate) busy: bool,
    pub(crate) keep_ready: bool,
    /// Free-text filter applied to the visible bucket.
    pub(crate) filter: String,
    /// Whether the action sheet is open.
    pub(crate) sheet: bool,
    pub(crate) toasts: Vec<Toast>,
    /// When the last successful sync landed, for the freshness read-out.
    pub(crate) synced_at: Option<Instant>,
    /// Who moved first, per account, for the origin arrow on each row.
    pub(crate) origins: HashMap<i64, Initiator>,
    /// Every recorded sync, oldest first.
    pub(crate) history: Vec<Snapshot>,
    /// The most recent changes, newest first.
    pub(crate) events: Vec<Event>,
    /// Which view is on screen.
    pub(crate) view: View,
    /// Which write is in flight, so its result can be described accurately.
    running: Option<Action>,
    following: Vec<User>,
    followers: Vec<User>,
    keep: Vec<i64>,
    github: Arc<Github>,
    store: Arc<Mutex<Option<Store>>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl GitbyeApp {
    /// Builds the application, applies the palette, and starts the first sync.
    ///
    /// A database that cannot be reached is reported and survived. Only a
    /// missing GitHub token is fatal, and that is handled before this point.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, github: Github) -> Self {
        theme::apply(&cc.egui_ctx);
        let (tx, rx) = channel();

        let mut app = Self {
            tab: Tab::Unreciprocated,
            buckets: Buckets::default(),
            selected: HashSet::new(),
            banner: None,
            progress: None,
            busy: false,
            keep_ready: false,
            filter: String::new(),
            sheet: false,
            toasts: Vec::new(),
            synced_at: None,
            origins: HashMap::new(),
            history: Vec::new(),
            events: Vec::new(),
            view: View::Accounts,
            running: None,
            following: Vec::new(),
            followers: Vec::new(),
            keep: Vec::new(),
            github: Arc::new(github),
            store: Arc::new(Mutex::new(None)),
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

    /// Runs a write against the current selection.
    ///
    /// The sheet that leads here has already named the count and listed every
    /// account, so this is the point of no return rather than another question.
    pub(crate) fn run(&mut self, action: Action, ctx: &Context) {
        let targets = self.selection();
        if targets.is_empty() {
            return;
        }
        let github = Arc::clone(&self.github);
        self.running = Some(action);
        self.sheet = false;
        self.spawn(ctx, move |reporter| {
            run_batch(&github, action, &targets, reporter)
        });
    }

    /// Follows back the accounts a toast is offering to restore.
    pub(crate) fn undo(&mut self, ctx: &Context, targets: Vec<User>) {
        let github = Arc::clone(&self.github);
        self.running = Some(Action::Follow);
        self.toasts.clear();
        self.spawn(ctx, move |reporter| {
            run_batch(&github, Action::Follow, &targets, reporter)
        });
    }

    /// Rows of the tab currently on screen, before filtering.
    pub(crate) fn rows(&self) -> &[User] {
        match self.tab {
            Tab::Unreciprocated => &self.buckets.unreciprocated,
            Tab::Keeping => &self.buckets.keeping,
            Tab::Mutuals => &self.buckets.mutuals,
            Tab::Fans => &self.buckets.fans,
        }
    }

    /// Rows actually on screen, once the filter has been applied.
    pub(crate) fn visible(&self) -> Vec<&User> {
        let needle = self.filter.trim().to_lowercase();
        self.rows()
            .iter()
            .filter(|user| needle.is_empty() || user.login.to_lowercase().contains(&needle))
            .collect()
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

    /// Adds a toast.
    pub(crate) fn toast(&mut self, message: String, tone: Tone, undo: Option<Vec<User>>) {
        self.toasts.push(Toast {
            message,
            tone,
            born: Instant::now(),
            undo,
        });
    }

    /// Turns a finished batch into a toast, offering undo where one is possible.
    ///
    /// The action is named in the past tense, matching the button that started
    /// it, so the same word follows the user through the whole flow.
    fn summarise(&mut self, action: Option<Action>, done: &[User], failed: &[(String, String)]) {
        let verb = action.map_or("Changed", Action::past);
        let undo = match action {
            // Following back is already the gentle direction, so it needs no
            // escape hatch. Only the destructive one gets an offer of reversal.
            Some(Action::Unfollow) if !done.is_empty() => Some(done.to_vec()),
            _ => None,
        };

        if failed.is_empty() {
            let message = format!("{verb} {}", plural(done.len(), "account"));
            self.toast(message, Tone::Good, undo);
            return;
        }

        let detail = failed
            .iter()
            .map(|(login, reason)| format!("{login} ({reason})"))
            .collect::<Vec<_>>()
            .join(", ");
        let message = format!(
            "{verb} {}, {} failed: {detail}",
            plural(done.len(), "account"),
            failed.len()
        );
        self.toast(message, Tone::Bad, undo);
    }

    /// Applies every message the workers have queued since the last frame.
    fn drain(&mut self, ctx: &Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Msg::Synced {
                    following,
                    followers,
                    recorded,
                } => {
                    // An unreadable store is a single fact, so the whole value
                    // is absent rather than each part being separately empty.
                    self.keep_ready = recorded.is_ok();
                    self.banner = recorded.as_ref().err().map(|reason| Banner {
                        message: format!(
                            "Keep-list unavailable: {reason}. Unfollowing is disabled."
                        ),
                        is_error: true,
                    });
                    let recorded = recorded.unwrap_or_default();
                    self.origins = recorded.origins;
                    self.history = recorded.history;
                    self.events = recorded.events;
                    self.keep = recorded.keep;
                    self.following = following;
                    self.followers = followers;
                    self.recompute();
                    self.busy = false;
                    self.progress = None;
                    self.synced_at = Some(Instant::now());
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
                Msg::Finished { done, failed } => {
                    let action = self.running.take();
                    self.summarise(action, &done, &failed);
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

impl eframe::App for GitbyeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        self.toasts.retain(|toast| !toast.expired());

        // A toast on screen is animating, so the frame must keep coming even
        // when nothing else has happened.
        if !self.toasts.is_empty() {
            ctx.request_repaint();
        }

        ui::draw(self, ctx);
    }
}
