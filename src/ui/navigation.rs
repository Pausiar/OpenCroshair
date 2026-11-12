//! Left sidebar: app name, page navigation with vector icons, overlay status at the bottom.
use crate::state::{AppState, Page};
use crate::ui::theme::{self, BLUE, BLUE_LIGHT, BLUE_TINT, SURFACE2, TEXT, TEXT2};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Rect, Response, Sense, Ui, Vec2};

const ITEMS: [(Page, &str, Icon); 5] = [
    (Page::Overview, "Overview", Icon::Overview),
    (Page::Editor, "Crosshair Editor", Icon::Editor),
    (Page::Presets, "Presets", Icon::Presets),
    (Page::Profiles, "Game Profiles", Icon::Profiles),
    (Page::Settings, "Settings", Icon::Settings),
];

/// Draw the sidebar contents (call inside a left panel).
pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(BLUE)
            .corner_radius(theme::RADIUS_SM)
            .inner_margin(egui::Margin::same(5))
            .show(ui, |ui| {
                widgets::icon(ui, Icon::Editor, 16.0, Color32::WHITE);
            });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("OpenCrosshair").size(16.0).strong().color(TEXT));
    });
    ui.add_space(22.0);

    for (page, label, icon) in ITEMS {
        let active = state.page == page;
        if nav_item(ui, label, icon, active).clicked() {
            state.page = page;
        }
        ui.add_space(2.0);
    }

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.add_space(12.0);
        overlay_status(ui, state);
        ui.add_space(8.0);
    });
}

fn nav_item(ui: &mut Ui, label: &str, icon: Icon, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let p = ui.painter();
        let bg = if active {
            BLUE_TINT
        } else if hovered {
            SURFACE2
        } else {
            Color32::TRANSPARENT
        };
        p.rect_filled(rect, theme::RADIUS_SM, bg);
        if active {
            let bar = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 8.0), Vec2::new(3.0, 20.0));
            p.rect_filled(bar, 2.0, BLUE);
        }
        let (icon_color, text_color) = if active {
            (BLUE_LIGHT, BLUE_LIGHT)
        } else if hovered {
            (TEXT, TEXT)
        } else {
            (TEXT2, TEXT2)
        };
        let icon_rect = Rect::from_min_size(rect.left_top() + Vec2::new(14.0, 8.0), Vec2::splat(20.0));
        widgets::paint_icon(p, icon_rect, icon, icon_color);
        p.text(
            rect.left_top() + Vec2::new(44.0, 18.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(14.0),
            text_color,
        );
    }
    resp
}

fn overlay_status(ui: &mut Ui, state: &AppState) {
    let (text, color) = if !state.settings.overlay_enabled {
        ("Overlay off", TEXT2)
    } else if !state.settings.crosshair_visible {
        ("Crosshair hidden", theme::DANGER)
    } else {
        ("Overlay active", theme::SUCCESS)
    };
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(theme::RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, color);
                ui.label(egui::RichText::new(text).size(13.0).color(TEXT));
            });
        });
}
