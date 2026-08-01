//! Hand-drawn pieces: the reciprocity glyph, account rows, action options and
//! toasts.
//!
//! These are painted rather than assembled from stock widgets, because the row
//! is the densest thing on screen and the glyph encodes the one fact the whole
//! application is about.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::model::{Initiator, User};
use crate::theme;

/// Height of one account row.
///
/// Comfortably past the 24px minimum pointer target, and near the 28px that
/// dense professional list interfaces settle on. The whole row is the target,
/// so the height is about legibility rather than about hitting a tick box.
pub(crate) const ROW_HEIGHT: f32 = 31.0;

/// Narrowest an account column may become before one is dropped.
///
/// A GitHub login runs to 39 characters but almost never does. This fits a
/// typical name plus the glyph and the link affordance, and gets one more
/// column onto the screen than a cautious width would.
pub const MIN_COLUMN: f32 = 204.0;

/// Most columns worth having. Past this the names are further apart than they
/// are tall, and the grid stops reading as a list.
pub const MAX_COLUMNS: usize = 5;

/// How many columns fit in `available` width.
///
/// Grown by accumulation rather than by dividing and rounding, so there is no
/// float-to-integer conversion to truncate, and so the arithmetic matches what
/// the layout actually does: place a column, then ask whether another still fits.
#[must_use]
pub fn column_count(available: f32, gap: f32) -> usize {
    let mut columns = 1;
    let mut used = MIN_COLUMN;

    while columns < MAX_COLUMNS && used + gap + MIN_COLUMN <= available {
        columns += 1;
        used += gap + MIN_COLUMN;
    }

    columns
}

/// Which way a relationship runs.
///
/// Drawn as two dots: the left is you, the right is them. A filled dot follows,
/// a hollow one does not. Every state in the application is one of the four
/// combinations, so the mark is complete rather than a selection of cases.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reciprocity {
    /// You follow them.
    pub(crate) outgoing: bool,
    /// They follow you.
    pub(crate) incoming: bool,
    /// Protected from unfollowing by the keep-list.
    pub(crate) shielded: bool,
    /// Who moved first, where that was observed.
    pub(crate) initiator: Initiator,
}

/// Draws the reciprocity mark inside `rect`.
///
/// The connector carries the direction the relationship started in, as an
/// arrowhead pointing away from whoever moved first. That keeps the origin in
/// the same mark as the reciprocity rather than adding a second badge beside it.
pub(crate) fn glyph(painter: &Painter, rect: Rect, mark: Reciprocity, tint: Color32) {
    let middle = rect.center().y;
    let left = pos2(rect.left() + 4.0, middle);
    let right = pos2(rect.right() - 4.0, middle);
    let radius = 3.2;

    painter.line_segment(
        [left, right],
        Stroke::new(1.2_f32, tint.gamma_multiply(0.45)),
    );
    origin_arrow(painter, rect.center(), mark.initiator, tint);

    for (centre, filled) in [(left, mark.outgoing), (right, mark.incoming)] {
        if filled {
            painter.circle_filled(centre, radius, tint);
            continue;
        }
        painter.circle_stroke(
            centre,
            radius,
            Stroke::new(1.4_f32, tint.gamma_multiply(0.7)),
        );
    }

    if mark.shielded {
        painter.circle_stroke(left, radius + 3.4, Stroke::new(1.2_f32, theme::AMBER));
    }
}

/// Draws the arrowhead showing which way the relationship began.
///
/// Nothing is drawn for an unobserved beginning, because an absent mark is the
/// honest rendering of an absent fact.
fn origin_arrow(painter: &Painter, centre: Pos2, initiator: Initiator, tint: Color32) {
    let direction = match initiator {
        Initiator::Me => 1.0_f32,
        Initiator::Them => -1.0_f32,
        Initiator::Unknown => return,
    };

    let length = 3.4;
    let half = 2.6;
    let tip = pos2(centre.x + direction * length, centre.y);
    let back = centre.x - direction * 0.6;

    painter.add(Shape::convex_polygon(
        vec![
            tip,
            pos2(back, centre.y - half),
            pos2(back, centre.y + half),
        ],
        tint.gamma_multiply(0.85),
        Stroke::NONE,
    ));
}

/// What the arrowhead means, in words, for the hover.
fn origin_hint(initiator: Initiator) -> Option<&'static str> {
    match initiator {
        Initiator::Me => Some("You followed first. The scheduled sweep never touches these."),
        Initiator::Them => Some("They followed first."),
        Initiator::Unknown => None,
    }
}

/// What a row is asking for.
#[derive(PartialEq, Eq)]
pub(crate) enum RowAction {
    Idle,
    Toggle,
    Open,
}

/// Draws one account.
///
/// The whole row is the selection target, because a tick box is a small target
/// beside a large piece of dead space. The profile link is a separate hot zone
/// at the trailing edge, revealed on hover so it does not compete at rest.
pub(crate) fn account_row(
    ui: &mut Ui,
    user: &User,
    tint: Color32,
    mark: Reciprocity,
    selected: bool,
    selectable: bool,
) -> RowAction {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    // `hovered` goes false while the pointer sits over the nested link target,
    // which would make the row highlight flicker off exactly when the cursor is
    // inside it. `contains_pointer` keeps every widget under the pointer.
    let hovered = response.contains_pointer();

    let open_rect = Rect::from_min_size(
        pos2(rect.right() - 28.0, rect.center().y - 10.0),
        Vec2::splat(20.0),
    );
    let open = ui.interact(open_rect, response.id.with("open"), Sense::click());

    paint_row_background(ui.painter(), rect, tint, selected, hovered, selectable);

    let painter = ui.painter();
    glyph(
        painter,
        Rect::from_min_size(
            pos2(rect.left() + 11.0, rect.center().y - 5.0),
            vec2(20.0, 10.0),
        ),
        mark,
        tint,
    );

    let label = if selected { theme::TEXT } else { theme::DIM };
    painter.text(
        pos2(rect.left() + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        &user.login,
        FontId::new(13.5, FontFamily::Name(theme::MONO.into())),
        if hovered { theme::TEXT } else { label },
    );

    if open.hovered() || hovered {
        painter.text(
            open_rect.center(),
            Align2::CENTER_CENTER,
            "\u{2197}",
            FontId::proportional(13.0),
            if open.hovered() {
                theme::AMBER
            } else {
                theme::FAINT
            },
        );
    }

    if let Some(hint) = origin_hint(mark.initiator) {
        response.clone().on_hover_text(hint);
    }

    if open.clicked() {
        return RowAction::Open;
    }
    if response.clicked() {
        // Read-only buckets have nothing to select, so a click there is a
        // request to look at the account rather than to act on it.
        return if selectable {
            RowAction::Toggle
        } else {
            RowAction::Open
        };
    }
    RowAction::Idle
}

/// Fill, outline and the selected marker behind a row.
fn paint_row_background(
    painter: &Painter,
    rect: Rect,
    tint: Color32,
    selected: bool,
    hovered: bool,
    selectable: bool,
) {
    let radius = CornerRadius::same(theme::RADIUS);
    let fill = match (selected, hovered) {
        (true, _) => tint.gamma_multiply(0.16),
        (false, true) => theme::HOVER,
        (false, false) => theme::RAISED,
    };
    painter.rect_filled(rect, radius, fill);

    if selected {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, tint.gamma_multiply(0.75)),
            StrokeKind::Inside,
        );
        // A solid edge on the leading side, so a selected row is legible from
        // the corner of the eye without reading its fill.
        let edge = Rect::from_min_size(
            rect.left_top() + vec2(0.0, 6.0),
            vec2(2.5, rect.height() - 12.0),
        );
        painter.rect_filled(edge, CornerRadius::same(2), tint);
        return;
    }

    if hovered && selectable {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, theme::LINE),
            StrokeKind::Inside,
        );
    }
}

/// Draws one choice inside the action sheet.
///
/// Options are cards rather than buttons because they carry a consequence as
/// well as a name, and a consequence needs a second line.
pub(crate) fn option_card(
    ui: &mut Ui,
    title: &str,
    detail: &str,
    tint: Color32,
    enabled: bool,
) -> Response {
    let height = 62.0;
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hovered = response.hovered() && enabled;
    let radius = CornerRadius::same(theme::RADIUS + 2);
    let painter = ui.painter();

    let wash = if enabled { 1.0 } else { 0.4 };
    painter.rect_filled(
        rect,
        radius,
        if hovered {
            tint.gamma_multiply(0.18)
        } else {
            theme::RAISED
        },
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(
            1.0_f32,
            tint.gamma_multiply(if hovered { 0.9 } else { 0.4 } * wash),
        ),
        StrokeKind::Inside,
    );

    painter.text(
        pos2(rect.left() + 18.0, rect.center().y - 10.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.5),
        tint.gamma_multiply(wash),
    );
    painter.text(
        pos2(rect.left() + 18.0, rect.center().y + 11.0),
        Align2::LEFT_CENTER,
        detail,
        FontId::proportional(12.5),
        theme::DIM.gamma_multiply(wash),
    );

    response
}
