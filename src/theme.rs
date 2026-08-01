//! The palette, the type scale, and their application to egui.
//!
//! The neutral scale is a warm violet-charcoal rather than a neutral grey. Pure
//! black and pure white are both absent: black surfaces make every other colour
//! look dirty by comparison, and pure white text on a dark field glares. Tinting
//! the darks slightly warm keeps the greys from reading as machine default.
//!
//! Colour is rationed. One amber carries every interactive state, so the eye
//! learns a single "this responds to you" signal. The four relationship tints
//! appear only on the glyph and the count, never on both a background and its
//! text, so no screen shows more than two hues at once.

use std::sync::Arc;

use eframe::egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style,
    TextStyle, Theme, ThemePreference, Visuals,
};

/// Deepest field, behind everything.
pub const BASE: Color32 = Color32::from_rgb(0x15, 0x14, 0x1B);
/// Panels and the navigation rail.
pub const SURFACE: Color32 = Color32::from_rgb(0x1C, 0x1A, 0x24);
/// Cards and rows at rest.
pub const RAISED: Color32 = Color32::from_rgb(0x24, 0x21, 0x2E);
/// Cards and rows under the cursor.
pub const HOVER: Color32 = Color32::from_rgb(0x2C, 0x28, 0x39);
/// Hairlines.
///
/// Deliberately translucent rather than a fixed grey. One token then composites
/// correctly over every surface in the ladder, instead of needing a different
/// value per depth, and it lands in the 1.3 to 1.5 to 1 band that reads as a
/// division without reading as a line.
pub const LINE: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);
/// Primary text. Contrast against [`SURFACE`] is 14.4 to 1.
pub const TEXT: Color32 = Color32::from_rgb(0xED, 0xE9, 0xF5);
/// Secondary text. Contrast 6.81 to 1.
pub const DIM: Color32 = Color32::from_rgb(0xA7, 0x9F, 0xBC);
/// Decorative only, never load-bearing text.
pub const FAINT: Color32 = Color32::from_rgb(0x6E, 0x67, 0x84);

/// The single interactive accent: focus, selection, primary action. 9.24 to 1.
pub const AMBER: Color32 = Color32::from_rgb(0xF0, 0xB3, 0x57);
/// Followed, not followed back. Also the destructive action. 5.65 to 1.
pub const SEVER: Color32 = Color32::from_rgb(0xE8, 0x70, 0x5A);
/// Mutual. 8.31 to 1.
pub const BOND: Color32 = Color32::from_rgb(0x58, 0xC7, 0xA9);
/// Follows you, unreturned. 6.62 to 1.
pub const INBOUND: Color32 = Color32::from_rgb(0x7E, 0x9C, 0xFF);
/// Protected by the keep-list. 6.71 to 1.
pub const SHIELD: Color32 = Color32::from_rgb(0xB9, 0x8B, 0xFF);

/// Corner softening. Small enough to read as precision, not as a toy.
pub const RADIUS: u8 = 6;

/// Family used for account names, which are identifiers and belong in mono.
pub const MONO: &str = "mono";

/// Family used where weight carries emphasis: counts, headings, primary actions.
pub const STRONG: &str = "strong";

/// Registers the bundled faces.
///
/// Inter carries prose and numerals. JetBrains Mono carries account names,
/// because a login is an identifier: fixed advance widths make a column of them
/// scannable, and the disambiguated shapes matter when a name is the thing you
/// are about to act on.
fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    let faces = [
        (
            "inter",
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            "inter-semibold",
            &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
        (
            "jetbrains",
            &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..],
        ),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains".to_owned());
    fonts.families.insert(
        FontFamily::Name(MONO.into()),
        vec!["jetbrains".to_owned(), "inter".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name(STRONG.into()),
        vec!["inter-semibold".to_owned(), "inter".to_owned()],
    );

    ctx.set_fonts(fonts);
}

/// Sets the type scale.
///
/// A fourth-based ramp rather than a uniform step, so the large numerals read as
/// a different class of information from the labels beside them instead of
/// merely a bigger version of them.
fn install_type(style: &mut Style) {
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(23.0)),
        (TextStyle::Body, FontId::proportional(15.0)),
        (TextStyle::Button, FontId::proportional(14.5)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(14.0)),
    ]
    .into();
}

/// Applies the palette and type scale. Call once, at startup.
pub fn apply(ctx: &Context) {
    install_fonts(ctx);

    let mut visuals = Visuals::dark();

    visuals.panel_fill = SURFACE;
    visuals.window_fill = RAISED;
    visuals.faint_bg_color = RAISED;
    visuals.extreme_bg_color = BASE;
    visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    visuals.window_corner_radius = CornerRadius::same(RADIUS * 2);
    visuals.selection.bg_fill = AMBER.gamma_multiply(0.22);
    visuals.selection.stroke = Stroke::new(1.0_f32, AMBER);
    visuals.hyperlink_color = AMBER;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = SEVER;
    // The default dark shadow is a black haze that muddies a violet ground.
    visuals.popup_shadow.color = Color32::from_black_alpha(120);
    visuals.window_shadow.color = Color32::from_black_alpha(140);

    for (widget, fill, text) in [
        (&mut visuals.widgets.noninteractive, SURFACE, DIM),
        (&mut visuals.widgets.inactive, RAISED, TEXT),
        (&mut visuals.widgets.hovered, HOVER, TEXT),
        (&mut visuals.widgets.active, HOVER, AMBER),
        (&mut visuals.widgets.open, HOVER, TEXT),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0_f32, LINE);
        widget.fg_stroke = Stroke::new(1.0_f32, text);
        widget.corner_radius = CornerRadius::same(RADIUS);
        widget.expansion = 0.0;
    }

    // egui keeps one style per theme and `set_visuals` writes only into the
    // active one, so a desktop-wide theme switch would otherwise strip all of
    // this and reveal an unstyled interface.
    ctx.set_theme(ThemePreference::Dark);
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_visuals_of(theme, visuals.clone());
    }
    ctx.all_styles_mut(install_type);
}
