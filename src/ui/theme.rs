//! Visual theme: palette tokens and egui style setup (dark, Linear-like, blue accent).
use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);
pub const SURFACE: Color32 = Color32::from_rgb(0x18, 0x1D, 0x25);
pub const SURFACE2: Color32 = Color32::from_rgb(0x20, 0x27, 0x35);
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x33, 0x40);
pub const BLUE: Color32 = Color32::from_rgb(0x3B, 0x82, 0xF6);
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(0x60, 0xA5, 0xFA);
pub const TEXT: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
pub const TEXT2: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
pub const SUCCESS: Color32 = Color32::from_rgb(0x27, 0xD9, 0x80);
pub const DANGER: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);
/// Tinted accent background for selected rows (blue at low alpha).
pub const BLUE_TINT: Color32 = Color32::from_rgba_premultiplied(0x1E, 0x3A, 0x6B, 0xFF);
/// Light preview background used when "light" preview is chosen.
pub const PREVIEW_LIGHT: Color32 = Color32::from_rgb(0xE5, 0xE7, 0xEB);
/// Dark preview background used for the crosshair preview.
pub const PREVIEW_DARK: Color32 = Color32::from_rgb(0x0B, 0x0E, 0x13);

pub const RADIUS: CornerRadius = CornerRadius::same(8);
pub const RADIUS_SM: CornerRadius = CornerRadius::same(6);

/// Install the dark theme on the context. Call once at startup.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = RADIUS;
    visuals.menu_corner_radius = RADIUS_SM;
    visuals.extreme_bg_color = Color32::from_rgb(0x0C, 0x0F, 0x14);
    visuals.faint_bg_color = SURFACE;
    visuals.code_bg_color = SURFACE2;
    visuals.hyperlink_color = BLUE_LIGHT;
    visuals.warn_fg_color = Color32::from_rgb(0xFB, 0xBF, 0x24);
    visuals.error_fg_color = DANGER;
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x3B, 0x82, 0xF6, 90);
    visuals.selection.stroke = Stroke::new(1.0, BLUE_LIGHT);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT2);
    w.noninteractive.corner_radius = RADIUS_SM;

    w.inactive.bg_fill = SURFACE2;
    w.inactive.weak_bg_fill = SURFACE2;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = RADIUS_SM;

    w.hovered.bg_fill = Color32::from_rgb(0x2A, 0x33, 0x45);
    w.hovered.weak_bg_fill = Color32::from_rgb(0x2A, 0x33, 0x45);
    w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3A, 0x46, 0x5A));
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.corner_radius = RADIUS_SM;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = BLUE;
    w.active.weak_bg_fill = BLUE;
    w.active.bg_stroke = Stroke::new(1.0, BLUE_LIGHT);
    w.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.active.corner_radius = RADIUS_SM;

    w.open.bg_fill = SURFACE2;
    w.open.weak_bg_fill = SURFACE2;
    w.open.bg_stroke = Stroke::new(1.0, BORDER);
    w.open.fg_stroke = Stroke::new(1.0, TEXT);
    w.open.corner_radius = RADIUS_SM;

    ctx.set_visuals(visuals);

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.combo_width = 220.0;
        style.spacing.slider_width = 180.0;
        style.text_styles = [
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
    });
}
