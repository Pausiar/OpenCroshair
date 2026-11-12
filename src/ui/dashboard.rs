//! Overview page: overlay status, active crosshair, active game profile, recent presets.
use crate::crosshair::model::Preset;
use crate::state::{AppState, Page};
use crate::ui::theme::{self, BLUE_TINT, BORDER, SURFACE2, TEXT, TEXT2};
use crate::ui::widgets::{self, paint_crosshair};
use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Ui, Vec2};

/// Draw the Overview page.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::page_header(
            ui,
            "Overview",
            "Your overlay, active crosshair and game profiles at a glance.",
        );

        let wide = ui.available_width() >= 780.0;
        if wide {
            ui.columns(2, |cols| {
                overlay_card(&mut cols[0], state);
                active_card(&mut cols[1], state);
            });
            ui.add_space(4.0);
            ui.columns(2, |cols| {
                profile_card(&mut cols[0], state);
                recent_card(&mut cols[1], state);
            });
        } else {
            overlay_card(ui, state);
            ui.add_space(4.0);
            active_card(ui, state);
            ui.add_space(4.0);
            profile_card(ui, state);
            ui.add_space(4.0);
            recent_card(ui, state);
        }
        ui.add_space(12.0);
    });
}

fn overlay_card(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(
        ui,
        "Overlay",
        Some("Draws the crosshair on top of your games. Click-through, never takes focus."),
        |ui| {
            let mut on = state.settings.overlay_enabled;
            ui.horizontal(|ui| {
                let (pill, color) = if on { ("Active", theme::SUCCESS) } else { ("Off", TEXT2) };
                ui.label(
                    RichText::new(if on { "Overlay is on" } else { "Overlay is off" })
                        .size(18.0)
                        .strong()
                        .color(TEXT),
                );
                widgets::status_pill(ui, pill, color);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::toggle_switch(ui, &mut on).changed() {
                        state.settings.overlay_enabled = on;
                        state.push_overlay();
                        state.dirty = true;
                    }
                });
            });
            ui.add_space(6.0);
            let hint = if !state.settings.crosshair_visible {
                "The crosshair is hidden, so nothing is drawn even when the overlay is on."
            } else {
                "The overlay follows the position settings in Settings > Overlay."
            };
            ui.label(RichText::new(hint).size(12.0).color(TEXT2));
        },
    );
}

fn active_card(ui: &mut Ui, state: &mut AppState) {
    let active_name = state
        .settings
        .active_preset_id
        .as_ref()
        .and_then(|id| state.presets.iter().find(|p| &p.id == id))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Custom crosshair".to_string());
    let c = state.settings.active_crosshair.clone();
    let zoom = state.settings.preview_zoom;
    let dark = state.settings.preview_dark_background;

    widgets::titled_card(ui, "Active crosshair", Some(&active_name), |ui| {
        ui.horizontal_top(|ui| {
            let size = 148.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            widgets::preview_box(ui, rect, dark, &c, zoom);

            ui.vertical(|ui| {
                ui.label(RichText::new(c.shape.label()).size(14.0).strong().color(TEXT));
                ui.add_space(4.0);
                let summary = format!(
                    "Length {:.0}px  Gap {:.0}px  Thickness {:.0}px",
                    c.length_h, c.gap, c.thickness
                );
                ui.label(RichText::new(summary).size(12.0).color(TEXT2));
                ui.horizontal(|ui| {
                    let (sw, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    let col = egui::Color32::from_rgba_unmultiplied(c.color.r, c.color.g, c.color.b, c.color.a);
                    ui.painter().rect_filled(sw, 3.0, col);
                    ui.label(RichText::new(c.color.to_hex()).size(12.0).color(TEXT2));
                });
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if widgets::primary_button(ui, "Open editor").clicked() {
                        state.page = Page::Editor;
                    }
                    let label = if state.settings.crosshair_visible {
                        "Hide crosshair"
                    } else {
                        "Show crosshair"
                    };
                    if widgets::secondary_button(ui, label).clicked() {
                        state.settings.crosshair_visible = !state.settings.crosshair_visible;
                        state.push_overlay();
                        state.dirty = true;
                    }
                });
            });
        });
    });
}

fn profile_card(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(
        ui,
        "Game profile",
        Some("Crosshair applied automatically while a profiled game is in front."),
        |ui| {
            if !state.settings.auto_profiles {
                ui.label(RichText::new("Auto profiles are off").size(15.0).color(TEXT));
                ui.label(
                    RichText::new("Turn them on in Settings > Game Detection, or press the auto-profiles hotkey.")
                        .size(12.0)
                        .color(TEXT2),
                );
                ui.add_space(8.0);
                if widgets::secondary_button(ui, "Open settings").clicked() {
                    state.page = Page::Settings;
                }
                return;
            }
            match state.active_profile.clone() {
                Some(name) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(16.0).strong().color(TEXT));
                        widgets::status_pill(ui, "Active", theme::SUCCESS);
                    });
                    ui.label(
                        RichText::new("Its preset is applied while this game is in the foreground.")
                            .size(12.0)
                            .color(TEXT2),
                    );
                }
                None => {
                    ui.label(RichText::new("No profiled game in front").size(15.0).color(TEXT));
                    ui.label(
                        RichText::new("The default crosshair is used. Profiles are matched by executable name.")
                            .size(12.0)
                            .color(TEXT2),
                    );
                }
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Manage profiles").clicked() {
                state.page = Page::Profiles;
            }
        },
    );
}

fn recent_card(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(
        ui,
        "Recent presets",
        Some("Click a preset to apply it to the overlay."),
        |ui| {
            let recent: Vec<Preset> = state
                .settings
                .recent_preset_ids
                .iter()
                .filter_map(|id| state.presets.iter().find(|p| &p.id == id).cloned())
                .take(8)
                .collect();
            if recent.is_empty() {
                ui.label(
                    RichText::new("No recent presets yet. Apply one from the Presets page.")
                        .size(13.0)
                        .color(TEXT2),
                );
                return;
            }
            let active_id = state.settings.active_preset_id.clone();
            for preset in recent {
                let is_active = active_id.as_deref() == Some(preset.id.as_str());
                if recent_row(ui, &preset, is_active).clicked() {
                    state.apply_preset(&preset.id);
                }
                ui.add_space(4.0);
            }
        },
    );
}

fn recent_row(ui: &mut Ui, preset: &Preset, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let fill = if active {
            BLUE_TINT
        } else if resp.hovered() {
            SURFACE2
        } else {
            theme::BG
        };
        let thumb = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 5.0), Vec2::splat(36.0));
        let bg_rect = rect;
        ui.painter().rect_filled(bg_rect, theme::RADIUS_SM, fill);
        ui.painter().rect_stroke(
            bg_rect,
            theme::RADIUS_SM,
            egui::Stroke::new(1.0, BORDER),
            egui::StrokeKind::Inside,
        );
        paint_crosshair(ui, thumb, &preset.crosshair, 1.0);
        let p = ui.painter();
        let x = rect.left() + 56.0;
        p.text(
            egui::pos2(x, rect.top() + 14.0),
            egui::Align2::LEFT_CENTER,
            &preset.name,
            egui::FontId::proportional(14.0),
            TEXT,
        );
        p.text(
            egui::pos2(x, rect.top() + 33.0),
            egui::Align2::LEFT_CENTER,
            &preset.category,
            egui::FontId::proportional(12.0),
            TEXT2,
        );
        if active {
            p.text(
                egui::pos2(rect.right() - 12.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "Active",
                egui::FontId::proportional(12.0),
                theme::BLUE_LIGHT,
            );
        }
    }
    resp
}
