//! The palette, and its application to egui.
//!
//! The neutral scale is achromatic: very light grey, grey, black. Colour appears
//! only in text, and only where it carries meaning. Every semantic colour clears
//! the WCAG AA contrast threshold of 4.5 to 1 against [`SURFACE`], and none of
//! them is the sole carrier of meaning, since every bucket also has its own tab
//! and heading.

use eframe::egui::{Color32, Context, CornerRadius, Stroke, Theme, ThemePreference, Visuals};

/// Window background, behind every panel.
pub const BACKGROUND: Color32 = Color32::from_rgb(0xED, 0xED, 0xED);
/// Panel surface, the colour most of the interface sits on.
pub const SURFACE: Color32 = Color32::from_rgb(0xF7, 0xF7, 0xF7);
/// Recessed areas such as scrolling list backgrounds.
pub const SUNKEN: Color32 = Color32::from_rgb(0xE0, 0xE0, 0xE0);
/// Hairlines and widget outlines.
pub const BORDER: Color32 = Color32::from_rgb(0xBF, 0xBF, 0xBF);
/// Primary text. Contrast against [`SURFACE`] is 17.2 to 1.
pub const INK: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
/// Secondary text. Contrast against [`SURFACE`] is 6.05 to 1.
pub const MUTED: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x5E);

/// Mutuals. Teal, contrast 5.78 to 1.
pub const RECIPROCATED: Color32 = Color32::from_rgb(0x0F, 0x6D, 0x63);
/// Not following back. Terracotta, contrast 5.60 to 1, complementary to the teal
/// because those two buckets are the central opposition of the application.
pub const UNRECIPROCATED: Color32 = Color32::from_rgb(0xA8, 0x43, 0x2A);
/// Fans. Indigo, contrast 7.04 to 1.
pub const INFORMATIONAL: Color32 = Color32::from_rgb(0x3B, 0x4C, 0xA8);
/// Keeping. Violet, contrast 7.13 to 1.
pub const PROTECTED: Color32 = Color32::from_rgb(0x6A, 0x3D, 0x9A);
/// Errors and failed rows. Red, contrast 7.04 to 1.
pub const FAILURE: Color32 = Color32::from_rgb(0xA3, 0x20, 0x20);

/// Tint behind a selected row. A desaturated indigo, so selection reads as
/// emphasis rather than as another semantic state.
const SELECTION: Color32 = Color32::from_rgb(0xCF, 0xD6, 0xEE);

/// Corner softening applied uniformly. Enough to avoid a hard technical look,
/// small enough not to read as decoration.
const RADIUS: u8 = 4;

/// Applies the palette to a context. Call once, at startup.
pub fn apply(ctx: &Context) {
    let mut visuals = Visuals::light();

    visuals.panel_fill = SURFACE;
    visuals.window_fill = SURFACE;
    visuals.faint_bg_color = BACKGROUND;
    visuals.extreme_bg_color = SUNKEN;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.selection.bg_fill = SELECTION;
    visuals.selection.stroke = Stroke::new(1.0_f32, INK);
    visuals.hyperlink_color = INFORMATIONAL;

    // Outline, text colour and corner softening are identical across states, so
    // only the fill varies. Varying the fill alone is what makes hovering legible
    // without the widget appearing to change shape.
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, SURFACE),
        (&mut visuals.widgets.inactive, BACKGROUND),
        (&mut visuals.widgets.hovered, SUNKEN),
        (&mut visuals.widgets.active, BORDER),
        (&mut visuals.widgets.open, SUNKEN),
    ] {
        // bg_fill covers widgets that must have a fill, such as a tick box.
        // weak_bg_fill covers those where it is optional, such as a button.
        // Setting only one leaves half the interface unstyled.
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, INK);
        widget.corner_radius = CornerRadius::same(RADIUS);
    }

    // egui keeps a separate style per theme and `set_visuals` writes only into
    // the currently active one. The default preference follows the desktop, so
    // without pinning the theme a system-wide switch to dark would drop this
    // palette entirely and reveal an unstyled interface.
    ctx.set_theme(ThemePreference::Light);
    ctx.set_visuals_of(Theme::Light, visuals.clone());
    ctx.set_visuals_of(Theme::Dark, visuals);
}
