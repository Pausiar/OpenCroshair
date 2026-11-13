//! First-run onboarding, drawn full window until `settings.onboarding_done` is set.
//! Three steps: welcome, pick a starting crosshair, and a ready step with hotkeys, tray, theme
//! and the overlay switch. Back, Next and Skip move between steps; Get started or Skip finishes.
use crate::crosshair::model::Preset;
use crate::state::AppState;
use crate::storage::config::ThemeMode;
use crate::ui::dashboard;
use crate::ui::theme;
use crate::ui::widgets;
use eframe::egui::{self, Align, Layout, RichText, Sense, Ui, Vec2};

/// Number of steps in the flow.
pub const STEP_COUNT: usize = 3;
const CARD_MAX_WIDTH: f32 = 640.0;
const STEP_KEY: &str = "oc_onboarding_step";
const TILE_COLUMNS: usize = 3;

/// Built-in presets offered on step 2, in display order.
pub const STARTER_IDS: [&str; 6] = [
    "builtin-minimal-hairline",
    "builtin-classic-green",
    "builtin-dot-white",
    "builtin-circle-ring",
    "builtin-precision-fine",
    "builtin-highvis-magenta",
];

const PRINCIPLES: [(&str, &str); 3] = [
    ("Free.", "There is nothing to buy or unlock."),
    ("Local.", "Settings, presets and profiles stay on this computer."),
    ("No telemetry.", "Nothing about your usage is collected or sent."),
];

/// Button pressed in the navigation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Back,
    Next,
    /// Get started or Skip: onboarding is finished.
    Done,
}

/// Keeps a step index inside the flow.
pub fn clamp_step(step: usize) -> usize {
    step.min(STEP_COUNT - 1)
}

/// Step index after a navigation press. `Done` leaves the index alone; the caller ends the flow.
pub fn step_after(step: usize, nav: Nav) -> usize {
    match nav {
        Nav::Back => clamp_step(step.saturating_sub(1)),
        Nav::Next => clamp_step(step + 1),
        Nav::Done => clamp_step(step),
    }
}

/// The starter presets from `presets`, in `STARTER_IDS` order. Missing ids are skipped.
pub fn pick_starters(presets: &[Preset]) -> Vec<&Preset> {
    STARTER_IDS
        .iter()
        .filter_map(|id| presets.iter().find(|p| p.id == *id))
        .collect()
}

/// Draw the onboarding flow.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    let step_id = widgets::state_id(STEP_KEY);
    let stored = ui.ctx().data_mut(|d| d.get_temp::<usize>(step_id)).unwrap_or(0);
    let step = clamp_step(stored);
    let mut nav = None;

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(48.0);
        dashboard::centered_column(ui, CARD_MAX_WIDTH, |ui| {
            widgets::card(ui, |ui| {
                progress_dots(ui, step);
                ui.add_space(20.0);
                match step {
                    0 => welcome_step(ui),
                    1 => starter_step(ui, state),
                    _ => ready_step(ui, state),
                }
                ui.add_space(24.0);
                nav = navigation_row(ui, step);
            });
        });
        ui.add_space(48.0);
    });

    let next = match nav {
        Some(Nav::Done) => {
            state.settings.onboarding_done = true;
            state.dirty = true;
            0
        }
        Some(n) => step_after(step, n),
        None => step,
    };
    ui.ctx().data_mut(|d| d.insert_temp(step_id, next));
}

/// Three dots for the steps: the current one is a wide pill, earlier steps are accent colored.
fn progress_dots(ui: &mut Ui, step: usize) {
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        for i in 0..STEP_COUNT {
            let active = i == step;
            let width = if active { 22.0 } else { 8.0 };
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
            let color = if active {
                p.primary
            } else if i < step {
                p.accent
            } else {
                p.border
            };
            ui.painter().rect_filled(rect, 4.0, color);
        }
        ui.add_space(8.0);
        widgets::caption(ui, &format!("Step {} of {}", step + 1, STEP_COUNT));
    });
}

fn welcome_step(ui: &mut Ui) {
    let p = theme::palette(ui.ctx());
    ui.label(widgets::heading_text("Welcome to OpenCrosshair", 28.0).color(p.text));
    ui.add_space(6.0);
    ui.label(
        RichText::new("A crosshair overlay for your games, ready in a minute.")
            .size(15.0)
            .color(p.text_muted),
    );
    ui.add_space(18.0);
    for (title, body) in PRINCIPLES {
        ui.horizontal_wrapped(|ui| {
            ui.label(widgets::semibold(title).size(15.0).color(p.text));
            ui.label(RichText::new(body).size(15.0).color(p.text_muted));
        });
        ui.add_space(6.0);
    }
}

fn starter_step(ui: &mut Ui, state: &mut AppState) {
    let p = theme::palette(ui.ctx());
    ui.label(widgets::heading_text("Pick a starting crosshair", 24.0).color(p.text));
    ui.add_space(4.0);
    ui.label(
        RichText::new("Click one to apply it. You can change it any time in the editor.")
            .size(14.0)
            .color(p.text_muted),
    );
    ui.add_space(16.0);

    let starters: Vec<Preset> = pick_starters(&state.presets).into_iter().cloned().collect();
    if starters.is_empty() {
        widgets::empty_state(
            ui,
            "No starter presets",
            "The built-in presets could not be found. You can still build a crosshair in the editor.",
        );
        return;
    }

    let gap = ui.spacing().item_spacing.x;
    let width = ((ui.available_width() - (TILE_COLUMNS as f32 - 1.0) * gap) / TILE_COLUMNS as f32).max(0.0);
    let active = state.settings.active_preset_id.clone();
    let mut clicked: Option<String> = None;
    for row in starters.chunks(TILE_COLUMNS) {
        ui.horizontal(|ui| {
            for preset in row {
                let selected = active.as_deref() == Some(preset.id.as_str());
                if dashboard::preset_tile(ui, preset, selected, width).clicked() {
                    clicked = Some(preset.id.clone());
                }
            }
        });
    }
    if let Some(id) = clicked {
        state.apply_preset(&id);
    }
}

fn ready_step(ui: &mut Ui, state: &mut AppState) {
    let p = theme::palette(ui.ctx());
    ui.label(widgets::heading_text("Ready", 24.0).color(p.text));
    ui.add_space(4.0);
    ui.label(
        RichText::new("A few things to know before you start.")
            .size(14.0)
            .color(p.text_muted),
    );

    widgets::section_label(ui, "Hotkeys");
    dashboard::hotkey_list(ui, &state.settings.hotkeys);

    widgets::section_label(ui, "Tray");
    ui.label(
        RichText::new(
            "With minimize to tray on, closing the window keeps OpenCrosshair running in the system tray. Quit from the tray menu.",
        )
        .size(13.0)
        .color(p.text_muted),
    );

    widgets::section_label(ui, "Theme");
    let mut mode = state.settings.theme;
    if widgets::segmented(ui, &mut mode, &[(ThemeMode::Light, "Light"), (ThemeMode::Dark, "Dark")]) {
        state.settings.theme = mode;
        theme::apply(ui.ctx(), mode);
        state.dirty = true;
    }

    ui.add_space(14.0);
    let mut on = state.settings.overlay_enabled;
    if ui
        .checkbox(&mut on, RichText::new("Enable overlay now").size(14.0).color(p.text))
        .changed()
    {
        state.settings.overlay_enabled = on;
        state.push_overlay();
        state.dirty = true;
    }
}

/// Back on the left of Next, Get started (or Next) on the right, Skip at the far left.
fn navigation_row(ui: &mut Ui, step: usize) -> Option<Nav> {
    let mut nav = None;
    ui.horizontal(|ui| {
        if widgets::ghost_button(ui, "Skip").clicked() {
            nav = Some(Nav::Done);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if step + 1 >= STEP_COUNT {
                if widgets::primary_button(ui, "Get started").clicked() {
                    nav = Some(Nav::Done);
                }
            } else if widgets::primary_button(ui, "Next").clicked() {
                nav = Some(Nav::Next);
            }
            if ui
                .add_enabled_ui(step > 0, |ui| widgets::secondary_button(ui, "Back"))
                .inner
                .clicked()
            {
                nav = Some(Nav::Back);
            }
        });
    });
    nav
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::presets::builtin_presets;

    #[test]
    fn clamp_step_stays_inside_flow() {
        assert_eq!(clamp_step(0), 0);
        assert_eq!(clamp_step(STEP_COUNT - 1), STEP_COUNT - 1);
        assert_eq!(clamp_step(99), STEP_COUNT - 1);
    }

    #[test]
    fn navigation_moves_and_stops_at_edges() {
        assert_eq!(step_after(0, Nav::Back), 0);
        assert_eq!(step_after(0, Nav::Next), 1);
        assert_eq!(step_after(1, Nav::Next), 2);
        assert_eq!(step_after(2, Nav::Next), 2);
        assert_eq!(step_after(2, Nav::Back), 1);
        assert_eq!(step_after(1, Nav::Done), 1);
    }

    #[test]
    fn starter_ids_all_exist_in_builtins_and_are_distinct() {
        let builtins = builtin_presets();
        let picked = pick_starters(&builtins);
        assert_eq!(picked.len(), STARTER_IDS.len());
        for (preset, id) in picked.iter().zip(STARTER_IDS) {
            assert_eq!(preset.id, id);
        }
        let mut ids = STARTER_IDS.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), STARTER_IDS.len());
    }

    #[test]
    fn pick_starters_skips_missing_presets_and_keeps_order() {
        let mut presets = builtin_presets();
        presets.retain(|p| p.id != STARTER_IDS[0] && p.id != STARTER_IDS[3]);
        let ids: Vec<&str> = pick_starters(&presets).iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, [STARTER_IDS[1], STARTER_IDS[2], STARTER_IDS[4], STARTER_IDS[5]]);
        assert!(pick_starters(&[]).is_empty());
    }
}
