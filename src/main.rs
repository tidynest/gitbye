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
use gitbye::app::GitbyeApp;
use gitbye::github::{self, Github};
use gitbye::sweep;

/// What the command line asked for.
enum Mode {
    Window,
    Sweep { rehearsal: bool },
    Help,
}

/// Reads the mode from the arguments.
///
/// Hand-rolled rather than pulled from a dependency, because there are three
/// options and none of them takes a value.
fn mode() -> Mode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| arguments.iter().any(|argument| argument == flag);

    match () {
        () if has("--help") || has("-h") => Mode::Help,
        () if has("--dry-run") => Mode::Sweep { rehearsal: true },
        () if has("--sweep") => Mode::Sweep { rehearsal: false },
        () => Mode::Window,
    }
}

const USAGE: &str = "\
gitbye - compare GitHub followers against following

    gitbye              open the window
    gitbye --sweep      run the unattended rule once, then exit
    gitbye --dry-run    say what --sweep would do, changing nothing
    gitbye --help       this

The sweep unfollows accounts that followed you, were followed back, and left
within ten weeks. It never touches a follow you began, an account on the
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
        Mode::Sweep { rehearsal } => {
            let report = sweep::run(rehearsal)?;
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
            .with_title("gitbye")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        // Honoured on X11. Inert on Wayland, where placement belongs to the
        // compositor, so the window rules do this instead.
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "gitbye",
        options,
        Box::new(move |cc| Ok(Box::new(GitbyeApp::new(cc, github)))),
    )
    .map_err(|error| anyhow!("could not open the window: {error}"))?;

    Ok(ExitCode::SUCCESS)
}
