//! Visual theme: palette tokens, WCAG contrast helper and egui style setup.
//!
//! Two palettes exist: [`LIGHT`] (cream surfaces, light-blue accent, dark-blue primary) and
//! [`DARK`] (deep navy). Pages read colors through [`palette`] so they follow the active mode.
use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Shadow, Stroke, TextStyle, Theme};

use crate::storage::config::ThemeMode;

/// Semantic color tokens for one theme mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_hover: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}

/// Light theme tokens.
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF5, 0xF2, 0xEA),
    surface: Color32::from_rgb(0xFB, 0xFA, 0xF6),
    surface_alt: Color32::from_rgb(0xEE, 0xEA, 0xE1),
    border: Color32::from_rgb(0xE0, 0xDA, 0xCE),
    text: Color32::from_rgb(0x1B, 0x2A, 0x41),
    text_muted: Color32::from_rgb(0x5B, 0x65, 0x73),
    // Darkened from #8A919C so it reaches 3:1 against bg (WCAG non-text/large-text floor).
    text_faint: Color32::from_rgb(0x7F, 0x86, 0x91),
    accent: Color32::from_rgb(0x6F, 0xA8, 0xDC),
    accent_soft: Color32::from_rgb(0xE3, 0xEE, 0xF8),
    primary: Color32::from_rgb(0x1F, 0x3A, 0x5F),
    on_primary: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    primary_hover: Color32::from_rgb(0x2A, 0x4C, 0x78),
    success: Color32::from_rgb(0x2F, 0x85, 0x5A),
    warning: Color32::from_rgb(0xB7, 0x79, 0x1F),
    danger: Color32::from_rgb(0xC2, 0x45, 0x3D),
};

/// Dark theme tokens.
pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x0F, 0x16, 0x26),
    surface: Color32::from_rgb(0x15, 0x1E, 0x31),
    surface_alt: Color32::from_rgb(0x1C, 0x27, 0x40),
    border: Color32::from_rgb(0x2A, 0x36, 0x52),
    text: Color32::from_rgb(0xEC, 0xE8, 0xDF),
    text_muted: Color32::from_rgb(0xA9, 0xB1, 0xC1),
    text_faint: Color32::from_rgb(0x7B, 0x84, 0x9A),
    accent: Color32::from_rgb(0x8C, 0xBC, 0xE8),
    accent_soft: Color32::from_rgb(0x1F, 0x33, 0x50),
    primary: Color32::from_rgb(0xCF, 0xE2, 0xF5),
    on_primary: Color32::from_rgb(0x0F, 0x16, 0x26),
    primary_hover: Color32::from_rgb(0xE1, 0xED, 0xF9),
    success: Color32::from_rgb(0x5C, 0xC2, 0x8A),
    warning: Color32::from_rgb(0xE0, 0xB0, 0x4F),
    danger: Color32::from_rgb(0xE5, 0x73, 0x68),
};

/// Palette for a configured mode.
pub fn palette_for(mode: ThemeMode) -> &'static Palette {
    match mode {
        ThemeMode::Light => &LIGHT,
        ThemeMode::Dark => &DARK,
    }
}

#[allow(dead_code)] // Pages not yet migrated to Palette; remove once they read it.
/// Palette of the mode currently active on the context (chosen by `visuals.dark_mode`).
pub fn palette(ctx: &egui::Context) -> &'static Palette {
    if ctx.global_style().visuals.dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

#[allow(dead_code)] // Used by the palette tests and by pages that check contrast at runtime.
/// WCAG 2.x relative luminance of an opaque color.
fn relative_luminance(c: Color32) -> f32 {
    fn channel(v: u8) -> f32 {
        let s = f32::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

#[allow(dead_code)] // Used by the palette tests and by pages that check contrast at runtime.
/// WCAG contrast ratio between two opaque colors, in the range 1.0 ..= 21.0.
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub const RADIUS: CornerRadius = CornerRadius::same(8);
pub const RADIUS_SM: CornerRadius = CornerRadius::same(6);

/// Install the theme for `mode` on the context. Call at startup and whenever the mode changes.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let p = palette_for(mode);
    let dark = matches!(mode, ThemeMode::Dark);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.dark_mode = dark;
    visuals.override_text_color = Some(p.text);
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_corner_radius = RADIUS;
    visuals.menu_corner_radius = RADIUS_SM;
    visuals.window_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 24 }),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 80 } else { 20 }),
    };
    visuals.extreme_bg_color = p.surface;
    visuals.faint_bg_color = p.surface_alt;
    visuals.code_bg_color = p.surface_alt;
    visuals.hyperlink_color = p.primary;
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.danger;
    visuals.selection.bg_fill = p.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, p.accent);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_muted);
    w.noninteractive.corner_radius = RADIUS_SM;

    w.inactive.bg_fill = p.surface;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = RADIUS;

    w.hovered.bg_fill = p.surface_alt;
    w.hovered.weak_bg_fill = p.surface_alt;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = RADIUS;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.primary;
    w.active.weak_bg_fill = p.primary;
    w.active.bg_stroke = Stroke::new(1.0, p.primary_hover);
    w.active.fg_stroke = Stroke::new(1.0, p.on_primary);
    w.active.corner_radius = RADIUS;

    w.open.bg_fill = p.surface_alt;
    w.open.weak_bg_fill = p.surface_alt;
    w.open.bg_stroke = Stroke::new(1.0, p.border);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = RADIUS;

    // Pin the theme so an OS dark/light change cannot swap the palette out from under the pages.
    let theme = if dark { Theme::Dark } else { Theme::Light };
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, visuals);

    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.combo_width = 220.0;
        style.spacing.slider_width = 180.0;
        style.text_styles = [
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Name("serif".into()))),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

// Legacy dark tokens. They are kept so pages not yet migrated to `Palette` still compile and
// render with the old look. Remove them once every page reads colors through `palette(ctx)`.
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const BG: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const SURFACE: Color32 = Color32::from_rgb(0x18, 0x1D, 0x25);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const SURFACE2: Color32 = Color32::from_rgb(0x20, 0x27, 0x35);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x33, 0x40);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const BLUE: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(0x60, 0xA5, 0xFA);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const TEXT: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const TEXT2: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const SUCCESS: Color32 = Color32::from_rgb(0x27, 0xD9, 0x80);
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const DANGER: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);
/// Tinted accent background for selected rows (blue at low alpha).
#[allow(dead_code)] // Legacy dark value; removed when pages migrate to Palette.
pub const BLUE_TINT: Color32 = Color32::from_rgba_premultiplied(0x1E, 0x3A, 0x6B, 0xFF);
/// Light preview background used when "light" preview is chosen.
#[allow(dead_code)] // Legacy preview value; removed when pages migrate to Palette.
pub const PREVIEW_LIGHT: Color32 = Color32::from_rgb(0xE5, 0xE7, 0xEB);
/// Dark preview background used for the crosshair preview.
#[allow(dead_code)] // Legacy preview value; removed when pages migrate to Palette.
pub const PREVIEW_DARK: Color32 = Color32::from_rgb(0x0B, 0x0E, 0x13);

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_TEXT: f32 = 4.5;
    const MIN_FAINT: f32 = 3.0;

    fn check(name: &str, fg: Color32, bg: Color32, min: f32) {
        let r = contrast_ratio(fg, bg);
        assert!(r >= min, "{name}: contrast {r:.2}:1 is below {min}:1");
    }

    #[test]
    fn contrast_ratio_matches_wcag_reference_values() {
        let black = Color32::BLACK;
        let white = Color32::WHITE;
        assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-6);
        assert!((contrast_ratio(black, white) - contrast_ratio(white, black)).abs() < 1e-6);
    }

    #[test]
    fn light_palette_meets_contrast_targets() {
        let p = &LIGHT;
        check("light text/bg", p.text, p.bg, MIN_TEXT);
        check("light text/surface", p.text, p.surface, MIN_TEXT);
        check("light text_muted/bg", p.text_muted, p.bg, MIN_TEXT);
        check("light on_primary/primary", p.on_primary, p.primary, MIN_TEXT);
        check("light text_faint/bg", p.text_faint, p.bg, MIN_FAINT);
    }

    #[test]
    fn dark_palette_meets_contrast_targets() {
        let p = &DARK;
        check("dark text/bg", p.text, p.bg, MIN_TEXT);
        check("dark text/surface", p.text, p.surface, MIN_TEXT);
        check("dark text_muted/bg", p.text_muted, p.bg, MIN_TEXT);
        check("dark on_primary/primary", p.on_primary, p.primary, MIN_TEXT);
        check("dark text_faint/bg", p.text_faint, p.bg, MIN_FAINT);
    }

    #[test]
    fn palette_for_selects_matching_mode() {
        assert_eq!(*palette_for(ThemeMode::Light), LIGHT);
        assert_eq!(*palette_for(ThemeMode::Dark), DARK);
    }
}
