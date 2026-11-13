//! Overview page: a hero card (live preview, overlay and crosshair switches, current preset and
//! profile), the recent presets strip and a hotkey cheat-sheet. Everything shown is live state.
use crate::crosshair::model::{Crosshair, Preset};
use crate::state::{AppState, Page};
use crate::storage::config::HotkeySettings;
use crate::ui::theme;
use crate::ui::widgets::{self, PreviewScene};
use eframe::egui::{
    self, Align, Align2, CursorIcon, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

/// Maximum width of the page content, in points.
pub const MAX_CONTENT_WIDTH: f32 = 980.0;
/// Horizontal and vertical page margins.
pub const PAGE_MARGIN_X: f32 = 32.0;
pub const PAGE_MARGIN_Y: f32 = 24.0;
/// Number of tiles in the recent presets strip.
const RECENT_LIMIT: usize = 6;
/// Side of the hero preview when the card is wide enough for two columns.
const PREVIEW_SIZE: f32 = 280.0;
/// Below this card width the hero stacks its preview above its controls.
const HERO_WIDE_MIN: f32 = 640.0;
const TILE_MIN_W: f32 = 96.0;
const TILE_MAX_W: f32 = 150.0;
const TILE_H: f32 = 116.0;
const TILE_THUMB_H: f32 = 72.0;
const TILE_RADIUS: u8 = 10;

/// Draw the Overview page.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(PAGE_MARGIN_Y);
        centered_column(ui, MAX_CONTENT_WIDTH, |ui| {
            widgets::page_header(
                ui,
                "Overview",
                "Your overlay, active crosshair and recent presets at a glance.",
            );
            hero_card(ui, state);
            ui.add_space(16.0);
            recent_card(ui, state);
            ui.add_space(16.0);
            hotkeys_card(ui, &state.settings.hotkeys);
        });
        ui.add_space(PAGE_MARGIN_Y);
    });
}

/// Runs `add` in a column centered in the available width. The column is at most `max_width`
/// wide and keeps the page margins on both sides.
pub fn centered_column(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui)) {
    let avail = ui.available_width();
    let width = (avail - 2.0 * PAGE_MARGIN_X).min(max_width).max(0.0);
    let pad = ((avail - width) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(width);
            add(ui);
        });
    });
}

fn hero_card(ui: &mut Ui, state: &mut AppState) {
    let crosshair = state.settings.active_crosshair.clone();
    let zoom = state.settings.preview_zoom;
    widgets::card(ui, |ui| {
        if ui.available_width() >= HERO_WIDE_MIN {
            ui.horizontal_top(|ui| {
                preview(ui, PREVIEW_SIZE, &crosshair, zoom);
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    hero_controls(ui, state);
                });
            });
        } else {
            let side = ui.available_width().min(PREVIEW_SIZE);
            ui.vertical(|ui| {
                preview(ui, side, &crosshair, zoom);
                ui.add_space(16.0);
                hero_controls(ui, state);
            });
        }
    });
}

fn preview(ui: &mut Ui, side: f32, crosshair: &Crosshair, zoom: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    widgets::preview_scene_box(ui, rect, PreviewScene::Dark, crosshair, zoom);
}

fn hero_controls(ui: &mut Ui, state: &mut AppState) {
    let p = theme::palette(ui.ctx());

    let mut overlay_on = state.settings.overlay_enabled;
    if switch_row(
        ui,
        "Overlay",
        "Draw the crosshair on top of your games. Click-through, never takes focus.",
        &mut overlay_on,
    ) {
        state.settings.overlay_enabled = overlay_on;
        state.push_overlay();
        state.dirty = true;
    }
    ui.add_space(8.0);

    let mut visible = state.settings.crosshair_visible;
    if switch_row(
        ui,
        "Crosshair visible",
        "Hide the crosshair without turning the overlay off.",
        &mut visible,
    ) {
        state.settings.crosshair_visible = visible;
        state.push_overlay();
        state.dirty = true;
    }

    widgets::divider(ui);

    widgets::section_label(ui, "Current preset");
    let name = active_preset_name(state.settings.active_preset_id.as_deref(), &state.presets);
    ui.label(widgets::heading_text(&name, 22.0).color(p.text));
    let c = &state.settings.active_crosshair;
    ui.label(
        RichText::new(format!("{} shape, {}", c.shape.label(), c.color.to_hex()))
            .size(13.0)
            .color(p.text_muted),
    );

    ui.add_space(12.0);
    widgets::section_label(ui, "Active profile");
    let profile = state.active_profile.as_deref().unwrap_or("None (default)");
    ui.label(widgets::semibold(profile).size(15.0).color(p.text));

    ui.add_space(20.0);
    if widgets::primary_button(ui, "Open editor").clicked() {
        state.page = Page::Editor;
    }
}

/// Label and description on the left, a switch on the right. Returns true when toggled.
fn switch_row(ui: &mut Ui, label: &str, description: &str, on: &mut bool) -> bool {
    let p = theme::palette(ui.ctx());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(widgets::semibold(label).size(16.0).color(p.text));
            ui.label(RichText::new(description).size(13.0).color(p.text_muted));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = widgets::toggle_switch(ui, on).changed();
        });
    });
    changed
}

fn recent_card(ui: &mut Ui, state: &mut AppState) {
    let recent = recent_presets(&state.settings.recent_preset_ids, &state.presets, RECENT_LIMIT);
    let active_id = state.settings.active_preset_id.clone();
    let mut clicked: Option<String> = None;

    widgets::titled_card(
        ui,
        "Recent presets",
        Some("Click a preset to apply it to the overlay."),
        |ui| {
            if recent.is_empty() {
                widgets::empty_state(
                    ui,
                    "No recent presets yet",
                    "Apply a preset from the Presets page and it will appear here.",
                );
                return;
            }
            let width = tile_width(ui.available_width(), ui.spacing().item_spacing.x, RECENT_LIMIT);
            ui.horizontal_wrapped(|ui| {
                for preset in &recent {
                    let selected = active_id.as_deref() == Some(preset.id.as_str());
                    if preset_tile(ui, preset, selected, width).clicked() {
                        clicked = Some(preset.id.clone());
                    }
                }
            });
        },
    );

    if let Some(id) = clicked {
        state.apply_preset(&id);
    }
}

/// One preset thumbnail with its name. Fixed size, so hovering never moves the layout.
pub fn preset_tile(ui: &mut Ui, preset: &Preset, selected: bool, width: f32) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, TILE_H), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hover = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("hover"), resp.hovered(), widgets::ANIM_TIME);
        let (fill, border, border_w, label_color) = if selected {
            (p.accent_soft, p.accent, 2.0, p.primary)
        } else {
            (
                widgets::mix_color(p.surface, p.surface_alt, hover),
                widgets::mix_color(p.border, p.accent, hover),
                1.0,
                p.text,
            )
        };
        ui.painter().rect_filled(rect, TILE_RADIUS, fill);
        ui.painter()
            .rect_stroke(rect, TILE_RADIUS, Stroke::new(border_w, border), StrokeKind::Inside);
        let thumb = Rect::from_min_size(rect.min + Vec2::splat(8.0), Vec2::new(width - 16.0, TILE_THUMB_H));
        widgets::preview_scene_box(ui, thumb, PreviewScene::Dark, &preset.crosshair, 1.0);
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - 14.0),
            Align2::CENTER_CENTER,
            &preset.name,
            FontId::proportional(13.0),
            label_color,
        );
    }
    resp
}

fn hotkeys_card(ui: &mut Ui, hotkeys: &HotkeySettings) {
    widgets::titled_card(
        ui,
        "Hotkeys",
        Some("Global shortcuts. They work while a game has focus."),
        |ui| hotkey_list(ui, hotkeys),
    );
}

/// One row per hotkey action: the action label on the left, its keys as chips on the right.
/// Shared with the first-run onboarding.
pub fn hotkey_list(ui: &mut Ui, hotkeys: &HotkeySettings) {
    let p = theme::palette(ui.ctx());
    for (label, keys) in hotkey_entries(hotkeys) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(RichText::new(label).size(14.0).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let parts = split_keys(keys);
                    if parts.is_empty() {
                        ui.label(RichText::new("Not set").size(13.0).color(p.text_faint));
                    }
                    // Right-to-left layout: add the last key first so it sits at the right edge.
                    for part in parts.iter().rev() {
                        widgets::key_chip(ui, part);
                    }
                });
            },
        );
    }
}

/// Action labels paired with their configured key strings, in display order.
pub fn hotkey_entries(h: &HotkeySettings) -> [(&'static str, &str); 5] {
    [
        ("Toggle crosshair", h.toggle_crosshair.as_str()),
        ("Toggle overlay", h.toggle_overlay.as_str()),
        ("Next preset", h.next_preset.as_str()),
        ("Previous preset", h.prev_preset.as_str()),
        ("Toggle auto profiles", h.toggle_auto_profiles.as_str()),
    ]
}

/// Splits a key string such as "Ctrl+Alt+F1" into its non-empty parts.
pub fn split_keys(keys: &str) -> Vec<&str> {
    keys.split('+').map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// Name of the preset with `active_id`, or "Custom crosshair" when none matches.
pub fn active_preset_name(active_id: Option<&str>, presets: &[Preset]) -> String {
    active_id
        .and_then(|id| presets.iter().find(|p| p.id == id))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Custom crosshair".to_string())
}

/// The presets named in `ids`, in that order, skipping ids that no longer exist, at most `limit`.
pub fn recent_presets(ids: &[String], presets: &[Preset], limit: usize) -> Vec<Preset> {
    ids.iter()
        .filter_map(|id| presets.iter().find(|p| &p.id == id))
        .take(limit)
        .cloned()
        .collect()
}

/// Width of one tile so that `count` tiles with `gap` between them fill `avail`, within limits.
pub fn tile_width(avail: f32, gap: f32, count: usize) -> f32 {
    let n = count.max(1) as f32;
    ((avail - (n - 1.0) * gap) / n).clamp(TILE_MIN_W, TILE_MAX_W)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::Preset;

    fn preset(id: &str, name: &str) -> Preset {
        Preset {
            id: id.into(),
            name: name.into(),
            ..Preset::default()
        }
    }

    #[test]
    fn recent_presets_keep_order_skip_missing_and_respect_limit() {
        let presets = vec![preset("a", "A"), preset("b", "B"), preset("c", "C")];
        let ids: Vec<String> = ["c", "gone", "a", "b"].iter().map(|s| s.to_string()).collect();
        let names: Vec<String> = recent_presets(&ids, &presets, 6).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["C", "A", "B"]);
        assert_eq!(recent_presets(&ids, &presets, 2).len(), 2);
        assert!(recent_presets(&[], &presets, 6).is_empty());
    }

    #[test]
    fn active_preset_name_falls_back_to_custom() {
        let presets = vec![preset("a", "Alpha")];
        assert_eq!(active_preset_name(Some("a"), &presets), "Alpha");
        assert_eq!(active_preset_name(Some("missing"), &presets), "Custom crosshair");
        assert_eq!(active_preset_name(None, &presets), "Custom crosshair");
    }

    #[test]
    fn split_keys_drops_empty_parts() {
        assert_eq!(split_keys("Ctrl+Alt+F1"), ["Ctrl", "Alt", "F1"]);
        assert_eq!(split_keys(" Ctrl + F2 "), ["Ctrl", "F2"]);
        assert!(split_keys("").is_empty());
    }

    #[test]
    fn hotkey_entries_are_in_display_order() {
        let h = HotkeySettings::default();
        let labels: Vec<&str> = hotkey_entries(&h).iter().map(|(l, _)| *l).collect();
        assert_eq!(labels.len(), 5);
        assert_eq!(hotkey_entries(&h)[0], ("Toggle crosshair", "Ctrl+Alt+F1"));
    }

    #[test]
    fn tile_width_fills_row_within_limits() {
        assert_eq!(tile_width(10_000.0, 8.0, 6), TILE_MAX_W);
        assert_eq!(tile_width(100.0, 8.0, 6), TILE_MIN_W);
        let w = tile_width(800.0, 8.0, 6);
        assert!((w * 6.0 + 5.0 * 8.0 - 800.0).abs() < 1e-3);
    }
}
