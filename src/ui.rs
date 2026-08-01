//! Rendering.
//!
//! Nothing here mutates the application beyond the tick boxes and the current
//! tab. Every other interaction is recorded as an [`Intent`] and applied once
//! the frame has been drawn, which keeps drawing and state changes separate.

use std::collections::HashSet;

use eframe::egui::{
    Align, Button, CentralPanel, Color32, Context, Frame, Id, Label, Layout, Margin, Modal,
    OpenUrl, ProgressBar, RichText, ScrollArea, Sense, TopBottomPanel, Ui,
};

use crate::app::{Action, GoodbyeApp, Tab};
use crate::model::User;
use crate::theme;

/// Something the user asked for, applied after the frame is drawn.
enum Intent {
    Sync,
    Keep,
    Unkeep,
    Ask(Action),
    Confirm,
    Cancel,
    SelectAll,
    SelectNone,
}

/// Builds a panel background at the given tone.
///
/// The chrome sits one step darker than the content it frames. Two neutral
/// tones separate controls from data more quietly than a border would, which is
/// why the window carries no dividing lines of its own.
fn panel_frame(fill: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(12, 8))
}

/// Draws one frame and applies whatever the user asked for.
pub(crate) fn draw(app: &mut GoodbyeApp, ctx: &Context) {
    let mut intent = None;

    header(app, ctx, &mut intent);
    tab_bar(app, ctx);
    action_bar(app, ctx, &mut intent);
    rows(app, ctx);
    confirmation(app, ctx, &mut intent);

    apply(app, ctx, intent);
}

/// Title, sync control, progress, and the banner.
fn header(app: &mut GoodbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    TopBottomPanel::top("header")
        .frame(panel_frame(theme::BACKGROUND))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("goodbye")
                        .size(20.0)
                        .strong()
                        .color(theme::INK),
                );
                ui.label(RichText::new("GitHub follow graph").color(theme::MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let sync = ui.add_enabled(!app.busy, Button::new("Sync"));
                    if sync.clicked() {
                        *intent = Some(Intent::Sync);
                    }
                });
            });

            if let Some(progress) = &app.progress {
                // Going through u16 keeps the conversion lossless rather than
                // silencing a precision lint. A batch cannot exceed 65535 accounts,
                // and saturating there would only affect the bar, never the work.
                let done = u16::try_from(progress.done).unwrap_or(u16::MAX);
                let total = u16::try_from(progress.total).unwrap_or(u16::MAX).max(1);
                let fraction = f32::from(done) / f32::from(total);
                let text = format!(
                    "{} of {}: {}",
                    progress.done, progress.total, progress.login
                );
                ui.add_space(6.0);
                ui.add(ProgressBar::new(fraction).text(text));
            }

            if let Some(banner) = &app.banner {
                let colour = if banner.is_error {
                    theme::FAILURE
                } else {
                    theme::RECIPROCATED
                };
                ui.add_space(6.0);
                ui.label(RichText::new(&banner.message).color(colour));
            }
        });
}

/// The four tabs, each carrying its own count.
fn tab_bar(app: &mut GoodbyeApp, ctx: &Context) {
    TopBottomPanel::top("tabs")
        .frame(panel_frame(theme::BACKGROUND))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    let label = format!("{}  ({})", tab.title(), app.count(tab));
                    let text = RichText::new(label).color(tab.colour());
                    if ui.selectable_label(app.tab == tab, text).clicked() {
                        app.tab = tab;
                        app.selected.clear();
                    }
                }
            });
        });
}

/// Selection helpers and the write actions available on the current tab.
fn action_bar(app: &mut GoodbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    if !app.tab.selectable() {
        return;
    }

    TopBottomPanel::bottom("actions")
        .frame(panel_frame(theme::BACKGROUND))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Select all").clicked() {
                    *intent = Some(Intent::SelectAll);
                }
                if ui.button("Clear").clicked() {
                    *intent = Some(Intent::SelectNone);
                }
                ui.label(
                    RichText::new(format!("{} selected", app.selected.len())).color(theme::MUTED),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    write_buttons(app, ui, intent);
                });
            });
        });
}

/// The per-tab write buttons. Kept apart so [`action_bar`] stays one level deep.
fn write_buttons(app: &GoodbyeApp, ui: &mut Ui, intent: &mut Option<Intent>) {
    let ready = !app.busy && !app.selected.is_empty();

    match app.tab {
        Tab::Unreciprocated => {
            let unfollow = ui.add_enabled(
                app.can_write(Action::Unfollow),
                Button::new(RichText::new("Unfollow selected").color(theme::UNRECIPROCATED)),
            );
            if unfollow.clicked() {
                *intent = Some(Intent::Ask(Action::Unfollow));
            }
            if ui
                .add_enabled(ready, Button::new("Keep selected"))
                .clicked()
            {
                *intent = Some(Intent::Keep);
            }
        }
        Tab::Keeping => {
            if ui.add_enabled(ready, Button::new("Stop keeping")).clicked() {
                *intent = Some(Intent::Unkeep);
            }
        }
        Tab::Fans => {
            let follow = ui.add_enabled(
                app.can_write(Action::Follow),
                Button::new(RichText::new("Follow selected").color(theme::INFORMATIONAL)),
            );
            if follow.clicked() {
                *intent = Some(Intent::Ask(Action::Follow));
            }
        }
        Tab::Mutuals => {}
    }
}

/// The scrolling list of accounts for the current tab.
fn rows(app: &mut GoodbyeApp, ctx: &Context) {
    let colour = app.tab.colour();
    let selectable = app.tab.selectable();

    CentralPanel::default()
        .frame(panel_frame(theme::SURFACE))
        .show(ctx, |ui| {
            if app.rows().is_empty() {
                ui.add_space(16.0);
                ui.label(RichText::new(empty_message(app)).color(theme::MUTED));
                return;
            }

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt("rows")
                .show(ui, |ui| {
                    // Borrowing the two fields separately keeps the row helper free of
                    // any knowledge of the application type.
                    let GoodbyeApp {
                        buckets,
                        selected,
                        tab,
                        ..
                    } = app;
                    let list = match tab {
                        Tab::Unreciprocated => &buckets.unreciprocated,
                        Tab::Keeping => &buckets.keeping,
                        Tab::Mutuals => &buckets.mutuals,
                        Tab::Fans => &buckets.fans,
                    };
                    for user in list {
                        row(ui, user, colour, selectable, selected);
                    }
                });
        });
}

/// What to say when a bucket is empty. Each answer is good news of a different
/// kind, so none of them reads as an error.
fn empty_message(app: &GoodbyeApp) -> &'static str {
    match app.tab {
        Tab::Unreciprocated if app.busy => "Loading.",
        Tab::Unreciprocated => "Everybody you follow follows you back.",
        Tab::Keeping => "Nobody is on the keep-list yet.",
        Tab::Mutuals => "No mutual follows.",
        Tab::Fans => "Nobody follows you that you do not already follow.",
    }
}

/// One account. The tick box appears only where a bulk action can act on it.
fn row(ui: &mut Ui, user: &User, colour: Color32, selectable: bool, selected: &mut HashSet<i64>) {
    ui.horizontal(|ui| {
        let mut ticked = selected.contains(&user.id);
        if selectable && ui.checkbox(&mut ticked, "").changed() {
            toggle(selected, user.id, ticked);
        }

        let label = Label::new(RichText::new(&user.login).color(colour)).sense(Sense::click());
        if ui.add(label).on_hover_text("Open on GitHub").clicked() {
            ui.ctx().open_url(OpenUrl::new_tab(format!(
                "https://github.com/{}",
                user.login
            )));
        }
    });
}

/// Adds or removes one id from the selection.
fn toggle(selected: &mut HashSet<i64>, id: i64, ticked: bool) {
    if ticked {
        selected.insert(id);
    } else {
        selected.remove(&id);
    }
}

/// The gate in front of every write. The visible list of accounts is the real
/// safety check, so it is shown in full rather than summarised.
fn confirmation(app: &GoodbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    let Some(confirm) = &app.confirm else {
        return;
    };

    let verb = confirm.action.verb();
    let response = Modal::new(Id::new("confirmation")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.heading(
            RichText::new(format!("{verb} {} accounts?", confirm.targets.len())).color(theme::INK),
        );
        ui.add_space(8.0);

        ScrollArea::vertical()
            .max_height(280.0)
            .id_salt("confirm_list")
            .show(ui, |ui| {
                for user in &confirm.targets {
                    ui.label(RichText::new(&user.login).color(theme::MUTED));
                }
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                *intent = Some(Intent::Cancel);
            }
            let go = ui.button(RichText::new(verb).color(theme::UNRECIPROCATED));
            if go.clicked() {
                *intent = Some(Intent::Confirm);
            }
        });
    });

    // Escape or a click on the backdrop dismisses, but never overrides a button
    // the user actually pressed.
    if response.should_close() && intent.is_none() {
        *intent = Some(Intent::Cancel);
    }
}

/// Carries out whatever the frame recorded.
fn apply(app: &mut GoodbyeApp, ctx: &Context, intent: Option<Intent>) {
    let Some(intent) = intent else {
        return;
    };

    match intent {
        Intent::Sync => app.sync(ctx),
        Intent::Keep => app.keep_selected(ctx),
        Intent::Unkeep => app.unkeep_selected(ctx),
        Intent::Ask(action) => app.ask(action),
        Intent::Confirm => app.run_confirmed(ctx),
        Intent::Cancel => app.confirm = None,
        Intent::SelectAll => {
            let ids: Vec<i64> = app.rows().iter().map(|user| user.id).collect();
            app.selected.extend(ids);
        }
        Intent::SelectNone => app.selected.clear(),
    }
}
