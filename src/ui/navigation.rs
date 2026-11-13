//! Left sidebar (220 px): serif wordmark, page navigation, and at the bottom the overlay status
//! pill with a quick switch. Colors come from the active [`theme::Palette`].
use crate::state::{AppState, Page};
use crate::ui::theme;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2};

const ITEMS: [(Page, &str, Icon); 5] = [
    (Page::Overview, "Overview", Icon::Overview),
    (Page::Editor, "Crosshair Editor", Icon::Editor),
    (Page::Presets, "Presets", Icon::Presets),
    (Page::Profiles, "Game Profiles", Icon::Profiles),
    (Page::Settings, "Settings", Icon::Settings),
];

/// Draw the sidebar contents (call inside a left panel with a surface fill).
pub fn show(ui: &mut Ui, state: &mut AppState) {
    paint_right_border(ui);
    wordmark(ui);
    ui.add_space(20.0);

    for (page, label, icon) in ITEMS {
        let selected = state.page == page;
        if widgets::nav_item(ui, icon, label, selected).clicked() {
            state.page = page;
        }
        ui.add_space(2.0);
    }

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        overlay_card(ui, state);
    });
}

/// 1 px divider on the sidebar's right edge.
fn paint_right_border(ui: &Ui) {
    let p = theme::palette(ui.ctx());
    let rect = ui.clip_rect();
    ui.painter()
        .vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, p.border));
}

/// Small logo mark followed by the serif app name.
fn wordmark(ui: &mut Ui) {
    let p = theme::palette(ui.ctx());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let (logo, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
        if ui.is_rect_visible(logo) {
            ui.painter().rect_filled(logo, 7.0, p.primary);
            widgets::paint_icon(ui.painter(), logo.shrink(5.0), Icon::Editor, p.on_primary);
        }
        ui.add_space(8.0);
        ui.label(widgets::heading_text("OpenCrosshair", 19.0).color(p.text));
    });
}

/// Overlay status pill plus a quick switch that turns the overlay on or off.
fn overlay_card(ui: &mut Ui, state: &mut AppState) {
    let p = theme::palette(ui.ctx());
    let (text, color) = if !state.settings.overlay_enabled {
        ("Overlay off", p.text_muted)
    } else if !state.settings.crosshair_visible {
        ("Crosshair hidden", p.danger)
    } else {
        ("Overlay active", p.success)
    };
    egui::Frame::new()
        .fill(p.surface_alt)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(theme::RADIUS_SM)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::status_pill(ui, text, color);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Overlay").size(14.0).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut on = state.settings.overlay_enabled;
                    if widgets::toggle_switch(ui, &mut on).changed() {
                        state.set_overlay_enabled(on);
                    }
                });
            });
        });
}
