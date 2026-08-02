//! Binary entry point.
//!
//! Two modes. With no arguments it opens the window. With `--sweep` it runs the
//! unattended rule once and exits, which is what the timer calls.
//!
//! Only a missing GitHub token is fatal to the window. A database that cannot be
//! reached is reported inside it instead, because the comparison still works
//! without one. The sweep is stricter and refuses outright.

use std::process::ExitCode;

use anyhow::{Result, anyhow};
use eframe::egui::ViewportBuilder;
use std::time::Duration;

use gitbye::app::GitbyeApp;
use gitbye::db::Store;
use gitbye::github::{self, Github};
use gitbye::model::{describe_grace, parse_grace};
use gitbye::sweep;

/// What the command line asked for.
enum Mode {
    Window,
    Sweep {
        rehearsal: bool,
        /// A window governing this run alone.
        grace: Option<Duration>,
    },
    SetGrace(Duration),
    Help,
    /// The arguments did not make sense, and this says why.
    Complaint(String),
}

/// The value given to a flag, written either as `--flag value` or `--flag=value`.
fn value_of(arguments: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    if let Some(joined) = arguments.iter().find_map(|a| a.strip_prefix(&prefix)) {
        return Some(joined.to_owned());
    }

    let at = arguments.iter().position(|argument| argument == flag)?;
    arguments.get(at + 1).cloned()
}

/// Reads the mode from the arguments.
///
/// Hand-rolled rather than pulled from a dependency, because there are a handful
/// of options and only two of them take a value.
fn mode() -> Mode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| arguments.iter().any(|argument| argument == flag);

    if has("--help") || has("-h") {
        return Mode::Help;
    }

    if let Some(spec) = value_of(&arguments, "--set-grace") {
        return match parse_grace(&spec) {
            Ok(window) => Mode::SetGrace(window),
            Err(reason) => Mode::Complaint(reason),
        };
    }

    let grace = match value_of(&arguments, "--grace").map(|spec| parse_grace(&spec)) {
        Some(Ok(window)) => Some(window),
        Some(Err(reason)) => return Mode::Complaint(reason),
        None => None,
    };

    match () {
        () if has("--dry-run") => Mode::Sweep {
            rehearsal: true,
            grace,
        },
        () if has("--sweep") => Mode::Sweep {
            rehearsal: false,
            grace,
        },
        // Accepting it here would look like it had been applied to something.
        () if grace.is_some() => Mode::Complaint(
            "--grace applies to --sweep or --dry-run. To change the stored window, use --set-grace"
                .to_owned(),
        ),
        () => Mode::Window,
    }
}

const USAGE: &str = "\
gitbye - compare GitHub followers against following

    gitbye                  open the window
    gitbye --sweep          run the unattended rule once, then exit
    gitbye --dry-run        say what --sweep would do, changing nothing
    gitbye --grace 6w       judge this run against a different window
    gitbye --set-grace 6w   change the stored window, then exit
    gitbye --help           this

A window is written in weeks or days: 6w, 45d, or a bare number of days. It may
be anything from 1 day to 365 days. --grace governs one run and leaves the
stored window alone, so a figure can be tried with --dry-run before adopting it.

The sweep unfollows accounts that followed you, were followed back, and left
within the window. It never touches a follow you began, an account on the
keep-list, or a relationship whose beginning predates this record.";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    match mode() {
        Mode::Help => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        Mode::Complaint(reason) => {
            eprintln!("{reason}");
            Ok(ExitCode::FAILURE)
        }
        Mode::SetGrace(window) => {
            let mut store = Store::connect()?;
            store.set_grace(window)?;
            println!("Sweep window set to {}.", describe_grace(window));
            Ok(ExitCode::SUCCESS)
        }
        Mode::Sweep { rehearsal, grace } => {
            let report = sweep::run(rehearsal, grace)?;
            println!("{}", report.describe());
            // A failure to unfollow somebody is worth a non-zero exit, so a
            // timer can report it rather than swallowing it.
            Ok(if report.failed.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Mode::Window => window(),
    }
}

/// Opens the window.
fn window() -> Result<ExitCode> {
    let github = Github::new(github::token()?)?;

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            // Wayland matches window rules on the app id, which is how the
            // compositor is told to float and centre this window. See the README.
            .with_app_id("gitbye")
            .with_title("GitBye")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        // Presenting a frame waits for the compositor to hand back a buffer, and
        // a window on a hidden workspace never gets one. Waiting for that inside
        // the event loop stops it answering the compositor's liveness pings, so
        // switching away from this window made it look hung. The interface is
        // static and repaints at most a few times a second, so nothing here
        // needs the tearing protection that waiting would buy.
        vsync: false,
        // Honoured on X11. Inert on Wayland, where placement belongs to the
        // compositor, so the window rules do this instead.
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "GitBye",
        options,
        Box::new(move |cc| Ok(Box::new(GitbyeApp::new(cc, github)))),
    )
    .map_err(|error| anyhow!("could not open the window: {error}"))?;

    Ok(ExitCode::SUCCESS)
}
