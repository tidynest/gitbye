//! Layout.
//!
//! Nothing here mutates the application except the filter text. Every other
//! interaction is recorded as an [`Intent`] and applied once the frame is drawn,
//! which keeps drawing and state changes apart.
//!
//! The window is a rail, a field, and a bar. The rail answers "how many", the
//! field answers "who", and the bar answers "what now". Each region owns exactly
//! one of those questions.

use eframe::egui::{
    Align, Align2, Area, Color32, Context, CornerRadius, FontFamily, FontId, Frame, Id, Key,
    Layout, Margin, Modal, Order, Rect, RichText, ScrollArea, Sense, SidePanel, Stroke, TextEdit,
    TopBottomPanel, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::app::{Action, GitbyeApp, Tab, Tone};
use crate::model::{Initiator, User};
use crate::theme;
use crate::widgets::{self, ROW_HEIGHT, Reciprocity, RowAction};

/// Width of the navigation rail.
const RAIL_WIDTH: f32 = 188.0;

/// Identifier of the filter field, so a shortcut can hand it focus.
fn filter_id() -> Id {
    Id::new("filter-field")
}

/// Something the user asked for, applied after the frame is drawn.
enum Intent {
    Sync,
    Show(Tab),
    Toggle(i64),
    SelectAll,
    SelectNone,
    ClearFilter,
    FocusFilter,
    OpenSheet,
    CloseSheet,
    Run(Action),
    Keep,
    Unkeep,
    Undo(Vec<User>),
    Dismiss(usize),
    Open(String),
}

/// A small tracked label. Letter spacing at this size stops short strings from
/// reading as a clump, and egui has no global setting for it.
fn eyebrow(text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .size(10.5)
        .extra_letter_spacing(1.1)
        .color(theme::FAINT)
}

/// Text in the emphasis family.
fn strong(text: impl Into<String>, size: f32, colour: Color32) -> RichText {
    RichText::new(text)
        .family(FontFamily::Name(theme::STRONG.into()))
        .size(size)
        .color(colour)
}

/// Draws one frame and applies whatever the user asked for.
pub(crate) fn draw(app: &mut GitbyeApp, ctx: &Context) {
    let mut intent = None;

    shortcuts(app, ctx, &mut intent);
    chrome(app, ctx, &mut intent);
    rail(app, ctx, &mut intent);
    action_bar(app, ctx, &mut intent);
    field(app, ctx, &mut intent);
    sheet(app, ctx, &mut intent);
    toasts(app, ctx, &mut intent);

    apply(app, ctx, intent);
}

/// Keyboard control.
///
/// Single-key shortcuts are suppressed while a text field has focus, otherwise
/// typing a name into the filter would trip half of them.
fn shortcuts(app: &GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    let typing = ctx.wants_keyboard_input();

    let (escape, find, all, digits, refresh, enter) = ctx.input(|i| {
        (
            i.key_pressed(Key::Escape),
            i.modifiers.command && i.key_pressed(Key::F),
            i.modifiers.command && i.key_pressed(Key::A),
            [Key::Num1, Key::Num2, Key::Num3, Key::Num4].map(|key| i.key_pressed(key)),
            i.key_pressed(Key::R),
            i.key_pressed(Key::Enter),
        )
    });

    if find {
        *intent = Some(Intent::FocusFilter);
        return;
    }
    if escape {
        // Escape peels one layer at a time rather than resetting everything.
        *intent = Some(match (app.sheet, app.filter.is_empty()) {
            (true, _) => Intent::CloseSheet,
            (false, false) => Intent::ClearFilter,
            (false, true) => Intent::SelectNone,
        });
        return;
    }
    if all && !app.sheet {
        *intent = Some(Intent::SelectAll);
        return;
    }
    if enter && !app.sheet && !app.selected.is_empty() {
        *intent = Some(Intent::OpenSheet);
        return;
    }
    if typing {
        return;
    }
    if refresh && !app.busy {
        *intent = Some(Intent::Sync);
        return;
    }
    for (index, pressed) in digits.into_iter().enumerate() {
        if pressed {
            *intent = Some(Intent::Show(Tab::ALL[index]));
        }
    }
}

/// Title, filter, freshness and the sync control.
fn chrome(app: &mut GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    let frame = Frame::new()
        .fill(theme::BASE)
        .inner_margin(Margin::symmetric(18, 14));

    TopBottomPanel::top("chrome").frame(frame).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(strong("gitbye", 19.0, theme::TEXT));
            ui.add_space(14.0);

            let field = TextEdit::singleline(&mut app.filter)
                .id(filter_id())
                .hint_text(RichText::new("Filter by name").color(theme::FAINT))
                .desired_width(260.0)
                .margin(Margin::symmetric(10, 6));
            ui.add(field);

            if !app.filter.is_empty() && ui.small_button("clear").clicked() {
                *intent = Some(Intent::ClearFilter);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let sync = ui.add_enabled(
                    !app.busy,
                    eframe::egui::Button::new(if app.busy { "Syncing" } else { "Sync" }),
                );
                if sync.clicked() {
                    *intent = Some(Intent::Sync);
                }
                ui.add_space(10.0);
                ui.label(RichText::new(freshness(app)).size(12.0).color(theme::FAINT));
            });
        });

        if let Some(banner) = &app.banner {
            ui.add_space(10.0);
            banner_strip(ui, &banner.message, banner.is_error);
        }
    });
}

/// How long ago the lists were refreshed, in words.
fn freshness(app: &GitbyeApp) -> String {
    let Some(at) = app.synced_at else {
        return if app.busy {
            "syncing".to_owned()
        } else {
            "not synced".to_owned()
        };
    };
    let seconds = at.elapsed().as_secs();
    match seconds {
        0..=5 => "just now".to_owned(),
        6..=59 => format!("{seconds}s ago"),
        60..=3599 => format!("{}m ago", seconds / 60),
        _ => format!("{}h ago", seconds / 3600),
    }
}

/// A persistent condition, drawn as a tinted strip rather than bare text so it
/// reads as a state of the window and not as another row of content.
fn banner_strip(ui: &mut Ui, message: &str, is_error: bool) {
    let tint = if is_error { theme::SEVER } else { theme::BOND };
    Frame::new()
        .fill(tint.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0_f32, tint.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.label(RichText::new(message).size(12.5).color(tint));
        });
}

/// The navigation rail: four buckets, each led by its count.
///
/// The count is the largest thing in the window because it is the reading the
/// application exists to give. The label underneath names it.
fn rail(app: &GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    let frame = Frame::new()
        .fill(theme::SURFACE)
        .inner_margin(Margin::symmetric(12, 14));

    SidePanel::left("rail")
        .frame(frame)
        .exact_width(RAIL_WIDTH)
        .resizable(false)
        .show(ctx, |ui| {
            for tab in Tab::ALL {
                if rail_entry(ui, tab, app.count(tab), tab == app.tab) {
                    *intent = Some(Intent::Show(tab));
                }
                ui.add_space(6.0);
            }
        });
}

/// One rail entry. Returns whether it was chosen.
fn rail_entry(ui: &mut Ui, tab: Tab, count: usize, active: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::click());
    let hot = response.contains_pointer();
    let tint = tab.colour();

    let ease = ui
        .ctx()
        .animate_bool_responsive(response.id.with("rail"), active || hot);
    let painter = ui.painter();

    let fill = theme::SURFACE.lerp_to_gamma(
        if active {
            tint.gamma_multiply(0.14)
        } else {
            theme::RAISED
        },
        ease,
    );
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS + 2), fill);

    if active {
        let edge = Rect::from_min_size(
            rect.left_top() + vec2(0.0, 10.0),
            vec2(3.0, rect.height() - 20.0),
        );
        painter.rect_filled(edge, CornerRadius::same(2), tint);
    }

    painter.text(
        pos2(rect.left() + 16.0, rect.top() + 24.0),
        Align2::LEFT_CENTER,
        count.to_string(),
        FontId::new(34.0, FontFamily::Name(theme::STRONG.into())),
        if active { tint } else { theme::TEXT },
    );
    painter.text(
        pos2(rect.left() + 16.0, rect.bottom() - 17.0),
        Align2::LEFT_CENTER,
        tab.title(),
        FontId::proportional(12.0),
        if active { theme::DIM } else { theme::FAINT },
    );

    response.clicked()
}

/// The account field: a responsive grid of rows.
fn field(app: &mut GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    let frame = Frame::new()
        .fill(theme::BASE)
        .inner_margin(Margin::symmetric(14, 12));

    eframe::egui::CentralPanel::default()
        .frame(frame)
        .show(ctx, |ui| {
            let rows = app.visible();
            if rows.is_empty() {
                empty_state(ui, app);
                return;
            }

            let tint = app.tab.colour();
            let selectable = app.tab.selectable();
            let gap = ui.spacing().item_spacing.x;
            let columns = widgets::column_count(ui.available_width(), gap);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt("field")
                .show(ui, |ui| {
                    for chunk in rows.chunks(columns) {
                        ui.columns(columns, |cells| {
                            for (index, user) in chunk.iter().enumerate() {
                                let origin = app
                                    .origins
                                    .get(&user.id)
                                    .copied()
                                    .unwrap_or(Initiator::Unknown);
                                let mark = mark_for(app.tab, origin);
                                let selected = app.selected.contains(&user.id);
                                let action = widgets::account_row(
                                    &mut cells[index],
                                    user,
                                    tint,
                                    mark,
                                    selected,
                                    selectable,
                                );
                                match action {
                                    RowAction::Toggle => *intent = Some(Intent::Toggle(user.id)),
                                    RowAction::Open => {
                                        *intent = Some(Intent::Open(user.login.clone()));
                                    }
                                    RowAction::Idle => {}
                                }
                            }
                        });
                    }
                    ui.add_space(ROW_HEIGHT);
                });
        });
}

/// Which relationship every row in a bucket has, by definition of the bucket.
fn mark_for(tab: Tab, initiator: Initiator) -> Reciprocity {
    let (outgoing, incoming, shielded) = match tab {
        Tab::Unreciprocated => (true, false, false),
        Tab::Keeping => (true, false, true),
        Tab::Mutuals => (true, true, false),
        Tab::Fans => (false, true, false),
    };

    Reciprocity {
        outgoing,
        incoming,
        shielded,
        initiator,
    }
}

/// What an empty field says. A filter that matches nothing is a different
/// situation from a bucket that is genuinely empty, and it gets a different line.
fn empty_state(ui: &mut Ui, app: &GitbyeApp) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        let filtered = !app.filter.trim().is_empty() && !app.rows().is_empty();
        let (headline, detail) = match (filtered, app.tab, app.busy) {
            (true, _, _) => ("No match", "Nothing in this list matches that filter."),
            (_, _, true) => ("Loading", "Reading your follow graph."),
            (_, Tab::Unreciprocated, _) => ("All square", "Everyone you follow follows you back."),
            (_, Tab::Keeping, _) => (
                "Nobody kept",
                "Select accounts and choose Keep to shield them.",
            ),
            (_, Tab::Mutuals, _) => ("No mutuals", "Nobody you follow follows you back yet."),
            (_, Tab::Fans, _) => (
                "No fans",
                "Nobody follows you that you do not already follow.",
            ),
        };
        ui.label(strong(headline, 17.0, theme::DIM));
        ui.add_space(6.0);
        ui.label(RichText::new(detail).size(13.0).color(theme::FAINT));
    });
}

/// The bar: what is selected, and the one way forward.
fn action_bar(app: &GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    if !app.tab.selectable() {
        return;
    }

    let frame = Frame::new()
        .fill(theme::SURFACE)
        .inner_margin(Margin::symmetric(18, 12));

    TopBottomPanel::bottom("bar").frame(frame).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if let Some(progress) = &app.progress {
                ui.label(
                    RichText::new(format!(
                        "{} of {}  {}",
                        progress.done, progress.total, progress.login
                    ))
                    .size(12.5)
                    .color(theme::AMBER),
                );
            } else {
                selection_summary(ui, app, intent);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if proceed_button(
                    ui,
                    app.selected.len(),
                    !app.selected.is_empty() && !app.busy,
                ) {
                    *intent = Some(Intent::OpenSheet);
                }
            });
        });
    });
}

/// Selection count, with the controls that change it.
fn selection_summary(ui: &mut Ui, app: &GitbyeApp, intent: &mut Option<Intent>) {
    let count = app.selected.len();
    let visible = app.visible().len();

    if count == 0 {
        ui.label(eyebrow(&format!("{visible} shown")));
        ui.add_space(10.0);
        if visible > 0 && ui.small_button("Select all").clicked() {
            *intent = Some(Intent::SelectAll);
        }
        return;
    }

    ui.label(strong(format!("{count} selected"), 14.0, theme::AMBER));
    ui.add_space(10.0);
    if ui.small_button("Clear").clicked() {
        *intent = Some(Intent::SelectNone);
    }
    if count < visible && ui.small_button("Select all").clicked() {
        *intent = Some(Intent::SelectAll);
    }
}

/// The single forward control. Everything it can lead to lives behind it.
fn proceed_button(ui: &mut Ui, count: usize, enabled: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        vec2(148.0, 38.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hot = response.contains_pointer() && enabled;
    let ease = ui
        .ctx()
        .animate_bool_responsive(response.id.with("proceed"), hot);

    let base = if enabled { theme::AMBER } else { theme::RAISED };
    let fill = base.gamma_multiply(if enabled { 0.88 + 0.12 * ease } else { 1.0 });
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS + 2), fill);

    let label = if count == 0 {
        "Proceed".to_owned()
    } else {
        format!("Proceed  {count}")
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(14.5, FontFamily::Name(theme::STRONG.into())),
        if enabled { theme::BASE } else { theme::FAINT },
    );

    response.clicked()
}

/// The action sheet.
///
/// This is the confirmation as well as the menu: it names the count, lists every
/// account by hand, and only then offers the choices. Splitting those into two
/// dialogues would ask twice and inform once.
fn sheet(app: &GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    if !app.sheet {
        return;
    }

    let selection = app.selection();
    let count = selection.len();
    let frame = Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0_f32, theme::LINE))
        .corner_radius(CornerRadius::same(theme::RADIUS * 2))
        .inner_margin(Margin::same(22));

    let response = Modal::new(Id::new("sheet"))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(170))
        .show(ctx, |ui| {
            ui.set_width(430.0);
            ui.label(eyebrow(&format!("{count} selected")));
            ui.add_space(4.0);
            ui.label(strong("What should happen to them?", 18.0, theme::TEXT));
            ui.add_space(14.0);

            sheet_names(ui, &selection);
            ui.add_space(16.0);

            for option in sheet_options(app.tab) {
                let enabled = option.enabled(app);
                if widgets::option_card(ui, option.title, option.detail, option.tint, enabled)
                    .clicked()
                {
                    *intent = Some(option.intent());
                }
                ui.add_space(8.0);
            }

            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                if ui.small_button("Cancel").clicked() {
                    *intent = Some(Intent::CloseSheet);
                }
            });
        });

    if response.should_close() && intent.is_none() {
        *intent = Some(Intent::CloseSheet);
    }
}

/// The names themselves, which are the real confirmation.
fn sheet_names(ui: &mut Ui, selection: &[User]) {
    Frame::new()
        .fill(theme::BASE)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("sheet-names")
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for user in selection {
                        ui.label(
                            RichText::new(&user.login)
                                .family(FontFamily::Name(theme::MONO.into()))
                                .size(12.5)
                                .color(theme::DIM),
                        );
                    }
                });
        });
}

/// One choice offered by the sheet.
struct SheetOption {
    title: &'static str,
    detail: &'static str,
    tint: Color32,
    action: SheetAction,
}

/// What choosing an option does.
#[derive(Clone, Copy)]
enum SheetAction {
    Unfollow,
    Follow,
    Keep,
    Unkeep,
}

impl SheetOption {
    /// Whether this choice can be taken right now.
    fn enabled(&self, app: &GitbyeApp) -> bool {
        match self.action {
            SheetAction::Unfollow => app.can_write(Action::Unfollow),
            SheetAction::Follow => app.can_write(Action::Follow),
            SheetAction::Keep | SheetAction::Unkeep => app.keep_ready && !app.busy,
        }
    }

    /// The intent it produces.
    fn intent(&self) -> Intent {
        match self.action {
            SheetAction::Unfollow => Intent::Run(Action::Unfollow),
            SheetAction::Follow => Intent::Run(Action::Follow),
            SheetAction::Keep => Intent::Keep,
            SheetAction::Unkeep => Intent::Unkeep,
        }
    }
}

/// Which choices belong to which bucket.
fn sheet_options(tab: Tab) -> Vec<SheetOption> {
    match tab {
        Tab::Unreciprocated => vec![
            SheetOption {
                title: "Unfollow",
                detail: "Stop following them. Reversible from the toast.",
                tint: theme::SEVER,
                action: SheetAction::Unfollow,
            },
            SheetOption {
                title: "Keep",
                detail: "Shield them, so they never appear in this list again.",
                tint: theme::SHIELD,
                action: SheetAction::Keep,
            },
        ],
        Tab::Keeping => vec![SheetOption {
            title: "Stop keeping",
            detail: "Drop the shield and return them to the unfollow list.",
            tint: theme::SEVER,
            action: SheetAction::Unkeep,
        }],
        Tab::Fans => vec![SheetOption {
            title: "Follow back",
            detail: "Return the follow, making it mutual.",
            tint: theme::INBOUND,
            action: SheetAction::Follow,
        }],
        Tab::Mutuals => Vec::new(),
    }
}

/// Toasts, stacked from the bottom-right.
///
/// They fade in quickly and out slowly: arrival should be noticed, departure
/// should not demand attention a second time.
fn toasts(app: &GitbyeApp, ctx: &Context, intent: &mut Option<Intent>) {
    if app.toasts.is_empty() {
        return;
    }

    Area::new(Id::new("toasts"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
        .show(ctx, |ui| {
            ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                for (index, toast) in app.toasts.iter().enumerate() {
                    // Enter slower than exit. Arrival should be noticed;
                    // departure should not ask for attention a second time.
                    let age = toast.born.elapsed().as_secs_f32();
                    let life = toast.life().as_secs_f32();
                    let opening = (age / 0.24).clamp(0.0, 1.0);
                    let closing = ((life - age) / 0.18).clamp(0.0, 1.0);

                    ui.scope(|ui| {
                        ui.set_opacity(opening.min(closing));
                        // Slide the last few pixels into place, which reads as
                        // arrival rather than as something blinking into being.
                        let rise = (1.0 - eframe::egui::emath::easing::cubic_out(opening)) * 14.0;
                        ui.add_space(-rise);
                        toast_card(ui, toast, index, intent);
                    });
                    ui.add_space(8.0);
                }
            });
        });
}

/// One toast.
fn toast_card(ui: &mut Ui, toast: &crate::app::Toast, index: usize, intent: &mut Option<Intent>) {
    let tint = match toast.tone {
        Tone::Good => theme::BOND,
        Tone::Bad => theme::SEVER,
    };

    let card = Frame::new()
        .fill(theme::RAISED)
        .stroke(Stroke::new(1.0_f32, tint.gamma_multiply(0.55)))
        .corner_radius(CornerRadius::same(theme::RADIUS + 2))
        .inner_margin(Margin::symmetric(14, 12))
        .shadow(eframe::egui::epaint::Shadow {
            offset: [0, 6],
            blur: 20,
            spread: 0,
            color: Color32::from_black_alpha(150),
        })
        .show(ui, |ui| {
            ui.set_max_width(356.0);
            ui.horizontal(|ui| {
                // A filled dot in the tone colour, so the outcome is readable
                // before the sentence is.
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, tint);
                ui.add_space(4.0);
                ui.label(RichText::new(&toast.message).size(13.0).color(theme::TEXT));
            });

            let Some(restore) = &toast.undo else {
                return;
            };
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let undo = ui.scope_builder(
                    UiBuilder::new()
                        .id_salt(Id::new("undo").with(index))
                        .sense(Sense::click()),
                    |ui| {
                        ui.label(strong("Undo", 13.0, theme::AMBER));
                    },
                );
                if undo.response.clicked() {
                    *intent = Some(Intent::Undo(restore.clone()));
                }
                ui.label(
                    RichText::new(format!("follow {} back", restore.len()))
                        .size(12.0)
                        .color(theme::FAINT),
                );
            });
        });

    // Clicking anywhere else on the card dismisses it, so a toast never has to
    // be waited out.
    if card.response.interact(Sense::click()).clicked() && intent.is_none() {
        *intent = Some(Intent::Dismiss(index));
    }
}

/// Carries out whatever the frame recorded.
fn apply(app: &mut GitbyeApp, ctx: &Context, intent: Option<Intent>) {
    let Some(intent) = intent else {
        return;
    };

    match intent {
        Intent::Sync => app.sync(ctx),
        Intent::Show(tab) => {
            app.tab = tab;
            app.selected.clear();
        }
        Intent::Toggle(id) => {
            if !app.selected.remove(&id) {
                app.selected.insert(id);
            }
        }
        Intent::SelectAll => {
            // Only what is on screen, so a filter narrows the blast radius
            // rather than hiding part of what a click is about to affect.
            let ids: Vec<i64> = app.visible().iter().map(|user| user.id).collect();
            app.selected.extend(ids);
        }
        Intent::SelectNone => app.selected.clear(),
        Intent::ClearFilter => app.filter.clear(),
        Intent::FocusFilter => ctx.memory_mut(|memory| memory.request_focus(filter_id())),
        Intent::OpenSheet => app.sheet = true,
        Intent::CloseSheet => app.sheet = false,
        Intent::Run(action) => app.run(action, ctx),
        Intent::Keep => {
            app.sheet = false;
            app.keep_selected(ctx);
        }
        Intent::Unkeep => {
            app.sheet = false;
            app.unkeep_selected(ctx);
        }
        Intent::Undo(targets) => app.undo(ctx, targets),
        Intent::Dismiss(index) => {
            if index < app.toasts.len() {
                app.toasts.remove(index);
            }
        }
        Intent::Open(login) => {
            ctx.open_url(eframe::egui::OpenUrl::new_tab(format!(
                "https://github.com/{login}"
            )));
        }
    }
}
