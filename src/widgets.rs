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

use crate::model::{Initiator, Snapshot, User};
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
/// Every state in the application is one of the four combinations of these, so
/// the mark drawn from it is complete rather than a selection of cases.
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

/// Draws the relationship mark inside `rect`.
///
/// A heart where affection exists, filled when it is returned and hollow when it
/// is only offered. A skull over crossed bones where it is one-sided and
/// unprotected, which is precisely the set the application exists to act on.
/// The same pair as the launcher icon, carrying the same meaning.
///
/// A small chevron ahead of the symbol points away from whoever moved first,
/// and is absent where the beginning was never observed.
pub(crate) fn glyph(painter: &Painter, rect: Rect, mark: Reciprocity, tint: Color32) {
    let centre = pos2(rect.right() - 8.0, rect.center().y);
    origin_arrow(
        painter,
        pos2(rect.left() + 2.0, centre.y),
        mark.initiator,
        tint,
    );

    // Followed, not followed back, not spared: the goodbye candidates.
    if mark.outgoing && !mark.incoming && !mark.shielded {
        skull(painter, centre, tint);
        return;
    }

    // Filled where the affection runs both ways or is deliberately kept,
    // hollow where it has been offered and not returned.
    heart(painter, centre, tint, mark.incoming || mark.shielded);

    if mark.shielded {
        painter.circle_stroke(
            centre,
            8.0,
            Stroke::new(1.1_f32, theme::AMBER.gamma_multiply(0.8)),
        );
    }
}

/// Sampled points of a heart, in a unit box centred on the origin.
///
/// From the classic parametric curve rather than hand-placed beziers, so the
/// shape stays symmetrical at any size and needs no control points to tune.
fn heart_outline(centre: Pos2, size: f32) -> Vec<Pos2> {
    const STEPS: usize = 36;
    let mut points = Vec::with_capacity(STEPS);

    for step in 0..STEPS {
        let t = f32::from(u16::try_from(step).unwrap_or(0)) / 36.0 * std::f32::consts::TAU;
        let x = 16.0 * t.sin().powi(3);
        let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        // The curve is drawn in maths orientation, so y is flipped for the screen.
        points.push(pos2(centre.x + x * size / 32.0, centre.y - y * size / 32.0));
    }

    points
}

/// Draws a heart, filled or outlined.
fn heart(painter: &Painter, centre: Pos2, tint: Color32, filled: bool) {
    let points = heart_outline(centre, 15.0);

    if filled {
        painter.add(Shape::convex_polygon(points, tint, Stroke::NONE));
        return;
    }
    painter.add(Shape::closed_line(
        points,
        Stroke::new(1.5_f32, tint.gamma_multiply(0.85)),
    ));
}

/// Draws a skull over crossed bones.
fn skull(painter: &Painter, centre: Pos2, tint: Color32) {
    let bone = tint.gamma_multiply(0.5);
    let reach = 6.4;

    for direction in [1.0_f32, -1.0] {
        painter.line_segment(
            [
                pos2(centre.x - reach, centre.y - reach * direction),
                pos2(centre.x + reach, centre.y + reach * direction),
            ],
            Stroke::new(2.0_f32, bone),
        );
    }

    // Cranium and jaw as one silhouette, then the sockets punched out of it.
    painter.circle_filled(centre + vec2(0.0, -1.0), 4.6, tint);
    painter.rect_filled(
        Rect::from_center_size(centre + vec2(0.0, 3.4), vec2(5.2, 3.4)),
        CornerRadius::same(1),
        tint,
    );
    for side in [-1.7_f32, 1.7] {
        painter.circle_filled(centre + vec2(side, -1.4), 1.5, theme::BASE);
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

    let length = 3.0;
    let half = 2.4;
    let tip = pos2(centre.x + direction * length, centre.y);
    let back = centre.x - direction * length;

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

    paint_row_background(ui.painter(), rect, tint, selected, hovered);

    let painter = ui.painter();
    glyph(
        painter,
        Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - 8.0),
            vec2(28.0, 16.0),
        ),
        mark,
        tint,
    );

    let label = if selected { theme::TEXT } else { theme::DIM };
    painter.text(
        pos2(rect.left() + 46.0, rect.center().y),
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
        return RowAction::Toggle;
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

    if hovered {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, theme::LINE),
            StrokeKind::Inside,
        );
    }
}

/// Draws the two counts over time.
///
/// Scaled to the range the data actually occupies rather than to zero. A
/// zero-based axis would render two nearly flat lines near the top of the box
/// and hide the only thing the plot is for, which is the change.
pub(crate) fn trend(ui: &mut Ui, history: &[Snapshot], height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), theme::RAISED);

    let Some((low, high)) = extent(history) else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Not enough history yet",
            FontId::proportional(12.5),
            theme::FAINT,
        );
        return;
    };

    // A gutter on the left so the range labels sit beside the plot rather than
    // on top of the line they describe.
    let plot = Rect::from_min_max(
        pos2(rect.left() + 46.0, rect.top() + 18.0),
        pos2(rect.right() - 16.0, rect.bottom() - 18.0),
    );
    for (series, tint) in [
        (Series::Followers, theme::BOND),
        (Series::Following, theme::SEVER),
    ] {
        let points: Vec<Pos2> = history
            .iter()
            .enumerate()
            .map(|(index, snapshot)| {
                let across = fraction(index, history.len().saturating_sub(1));
                let up =
                    f32::from(u16::try_from(series.of(snapshot).saturating_sub(low)).unwrap_or(0))
                        / f32::from(u16::try_from(high - low).unwrap_or(1).max(1));
                pos2(
                    plot.left() + across * plot.width(),
                    plot.bottom() - up * plot.height(),
                )
            })
            .collect();

        painter.add(Shape::line(points.clone(), Stroke::new(1.8_f32, tint)));
        if let Some(last) = points.last() {
            painter.circle_filled(*last, 3.0, tint);
        }
    }

    for (value, y) in [(high, plot.top()), (low, plot.bottom())] {
        painter.text(
            pos2(plot.left() - 10.0, y),
            Align2::RIGHT_CENTER,
            format!("{value}"),
            FontId::proportional(10.5),
            theme::FAINT,
        );
    }
}

/// Which line is being drawn.
#[derive(Clone, Copy)]
enum Series {
    Following,
    Followers,
}

impl Series {
    /// Reads this series out of a snapshot.
    fn of(self, snapshot: &Snapshot) -> usize {
        match self {
            Self::Following => snapshot.following,
            Self::Followers => snapshot.followers,
        }
    }
}

/// Lowest and highest value across both series, or `None` when there is not
/// enough history to draw a line.
fn extent(history: &[Snapshot]) -> Option<(usize, usize)> {
    if history.len() < 2 {
        return None;
    }

    let values = history
        .iter()
        .flat_map(|snapshot| [snapshot.following, snapshot.followers]);
    let low = values.clone().min()?;
    let high = values.max()?;

    // A perfectly flat history would divide by zero, so widen it by one.
    Some(if low == high {
        (low, high + 1)
    } else {
        (low, high)
    })
}

/// Position of one point along the horizontal axis, without a lossy cast.
fn fraction(index: usize, last: usize) -> f32 {
    if last == 0 {
        return 0.0;
    }
    let index = u16::try_from(index).unwrap_or(u16::MAX);
    let last = u16::try_from(last).unwrap_or(u16::MAX).max(1);
    f32::from(index) / f32::from(last)
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
