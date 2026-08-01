//! Binary entry point.
//!
//! Only a missing GitHub token is fatal here. A database that cannot be reached
//! is reported inside the window instead, because the comparison itself still
//! works without it.

use anyhow::{Result, anyhow};
use eframe::egui::ViewportBuilder;
use goodbye::app::GoodbyeApp;
use goodbye::github::{self, Github};

fn main() -> Result<()> {
    let github = Github::new(github::token()?)?;

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            // Wayland matches window rules on the app id, which is how the
            // compositor is told to float and centre this window. See the README.
            .with_app_id("goodbye")
            .with_title("goodbye")
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([620.0, 420.0]),
        // Honoured on X11. Inert on Wayland, where placement belongs to the
        // compositor, so the window rules do this instead.
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "goodbye",
        options,
        Box::new(move |cc| Ok(Box::new(GoodbyeApp::new(cc, github)))),
    )
    .map_err(|error| anyhow!("could not open the window: {error}"))
}
