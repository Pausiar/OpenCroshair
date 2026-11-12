//! Settings page with category tabs: General, Overlay, Hotkeys, Appearance, Detection, Startup, About.
use crate::hotkeys::capture_hotkey_string;
use crate::state::AppState;
use crate::storage::config::{CenterMode, HotkeySettings, Settings};
use crate::system::monitors::{self, MonitorInfo};
use crate::ui::theme::{self, TEXT, TEXT2};
use crate::ui::widgets::{self, state_id};
use eframe::egui::{self, Align, Layout, RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    Overlay,
    Hotkeys,
    Appearance,
    Detection,
    Startup,
    About,
}

const TABS: [(Tab, &str); 7] = [
    (Tab::General, "General"),
    (Tab::Overlay, "Overlay"),
    (Tab::Hotkeys, "Hotkeys"),
    (Tab::Appearance, "Appearance"),
    (Tab::Detection, "Detection"),
    (Tab::Startup, "Startup"),
    (Tab::About, "About"),
];

/// Hotkey rows: (settings key, label, description). Keys match `app::action_key`.
pub const HOTKEY_ROWS: [(&str, &str, &str); 5] = [
    ("toggle_crosshair", "Toggle crosshair", "Show or hide the crosshair without changing the overlay."),
    ("toggle_overlay", "Toggle overlay", "Turn the overlay window on or off."),
    ("next_preset", "Next preset", "Apply the next favorite preset (or any preset if none are favorites)."),
    ("prev_preset", "Previous preset", "Apply the previous favorite preset (or any preset if none are favorites)."),
    ("toggle_auto_profiles", "Auto profiles", "Turn automatic game profiles on or off."),
];

const TAB_ID: &str = "oc_settings_tab";
const CAPTURE_ID: &str = "oc_settings_hotkey_capture";
const CONFIRM_RESET_ID: &str = "oc_settings_confirm_reset";
const MONITORS_ID: &str = "oc_settings_monitors";

/// Draw the Settings page.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    let mut tab: Tab = ui.ctx().data(|d| d.get_temp(state_id(TAB_ID))).unwrap_or_default();
    widgets::page_header(ui, "Settings", "Window, overlay, hotkeys, appearance, detection and startup options.");
    if widgets::segmented(ui, &mut tab, &TABS.map(|(t, l)| (t, l))) {
        // Leaving the hotkey tab cancels any pending capture.
        ui.ctx().data_mut(|d| d.remove::<Option<&'static str>>(state_id(CAPTURE_ID)));
    }
    ui.ctx().data_mut(|d| d.insert_temp(state_id(TAB_ID), tab));
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match tab {
            Tab::General => general_tab(ui, state),
            Tab::Overlay => overlay_tab(ui, state),
            Tab::Hotkeys => hotkeys_tab(ui, state),
            Tab::Appearance => appearance_tab(ui, state),
            Tab::Detection => detection_tab(ui, state),
            Tab::Startup => startup_tab(ui, state),
            Tab::About => about_tab(ui),
        }
        ui.add_space(12.0);
    });
}

fn general_tab(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(ui, "Window", Some("How the main window behaves."), |ui| {
        if widgets::toggle_row(
            ui,
            "Minimize to tray",
            Some("Closing the window keeps OpenCrosshair running in the system tray. Quit from the tray menu."),
            &mut state.settings.minimize_to_tray,
        ) {
            state.dirty = true;
        }
        ui.add_space(6.0);
        if widgets::toggle_row(
            ui,
            "Start minimized",
            Some("Start hidden in the tray. Also applies when launched with --minimized."),
            &mut state.settings.start_minimized,
        ) {
            state.dirty = true;
        }
    });
    ui.add_space(4.0);

    widgets::titled_card(
        ui,
        "Reset",
        Some("Restore every setting to its default value. Presets and game profiles are kept."),
        |ui| {
            let confirming = ui
                .ctx()
                .data(|d| d.get_temp::<bool>(state_id(CONFIRM_RESET_ID)))
                .unwrap_or(false);
            ui.horizontal(|ui| {
                if !confirming {
                    if widgets::danger_button(ui, "Reset settings to defaults").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(state_id(CONFIRM_RESET_ID), true));
                    }
                } else {
                    ui.label(RichText::new("Reset all settings?").color(TEXT));
                    if widgets::danger_button(ui, "Confirm reset").clicked() {
                        reset_settings(state);
                        ui.ctx().data_mut(|d| d.insert_temp(state_id(CONFIRM_RESET_ID), false));
                    }
                    if widgets::secondary_button(ui, "Cancel").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(state_id(CONFIRM_RESET_ID), false));
                    }
                }
            });
        },
    );
}

/// Restore `Settings::default()` and flag the side effects (hotkeys, startup, overlay).
fn reset_settings(state: &mut AppState) {
    let old = state.settings.clone();
    let new = Settings::default();
    state.hotkeys_changed = old.hotkeys != new.hotkeys;
    state.startup_changed = old.launch_at_startup != new.launch_at_startup;
    state.editing = new.active_crosshair.clone();
    state.settings = new;
    state.active_profile = None;
    state.push_overlay();
    state.dirty = true;
    state.toast("Settings restored to defaults", false);
}

fn overlay_tab(ui: &mut Ui, state: &mut AppState) {
    let monitors = cached_monitors(ui);
    let mut changed = false;

    widgets::titled_card(
        ui,
        "Position",
        Some("Where the crosshair is drawn on screen."),
        |ui| {
            ui.label(RichText::new("Anchor").size(13.0).color(TEXT2));
            let mut mode = state.settings.position.mode;
            if widgets::segmented(
                ui,
                &mut mode,
                &[
                    (CenterMode::Monitor, "Monitor center"),
                    (CenterMode::GameWindow, "Game window center"),
                    (CenterMode::Custom, "Custom position"),
                ],
            ) {
                state.settings.position.mode = mode;
                changed = true;
            }
            ui.add_space(10.0);

            match state.settings.position.mode {
                CenterMode::Monitor | CenterMode::GameWindow => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Monitor").color(TEXT).size(14.0));
                        let current = state.settings.position.monitor;
                        let selected = monitor_label(current, &monitors);
                        let mut pick = current;
                        egui::ComboBox::from_id_salt("oc_overlay_monitor")
                            .selected_text(selected)
                            .width(360.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut pick, None, "Primary monitor");
                                for m in &monitors {
                                    ui.selectable_value(&mut pick, Some(m.index), monitor_text(m));
                                }
                            });
                        if pick != current {
                            state.settings.position.monitor = pick;
                            changed = true;
                        }
                        if widgets::secondary_button(ui, "Refresh").clicked() {
                            ui.ctx().data_mut(|d| d.remove::<Vec<MonitorInfo>>(state_id(MONITORS_ID)));
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new("Offset from the anchor, in pixels").size(12.0).color(TEXT2));
                    ui.horizontal(|ui| {
                        changed |= int_field(ui, "X", &mut state.settings.position.offset_x, -2000, 2000);
                        ui.add_space(16.0);
                        changed |= int_field(ui, "Y", &mut state.settings.position.offset_y, -2000, 2000);
                    });
                }
                CenterMode::Custom => {
                    ui.label(
                        RichText::new("Absolute screen position of the crosshair center, in physical pixels.")
                            .size(12.0)
                            .color(TEXT2),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        changed |= int_field(ui, "X", &mut state.settings.position.custom_x, -20000, 20000);
                        ui.add_space(16.0);
                        changed |= int_field(ui, "Y", &mut state.settings.position.custom_y, -20000, 20000);
                    });
                }
            }
        },
    );

    if changed {
        state.push_overlay();
        state.dirty = true;
    }
}

fn cached_monitors(ui: &mut Ui) -> Vec<MonitorInfo> {
    let id = state_id(MONITORS_ID);
    if let Some(list) = ui.ctx().data(|d| d.get_temp::<Vec<MonitorInfo>>(id)) {
        return list;
    }
    let list = monitors::list_monitors();
    ui.ctx().data_mut(|d| d.insert_temp(id, list.clone()));
    list
}

fn monitor_text(m: &MonitorInfo) -> String {
    let primary = if m.primary { " (primary)" } else { "" };
    format!("{}: {} {}x{}{}", m.index + 1, m.name, m.width, m.height, primary)
}

fn monitor_label(selected: Option<usize>, monitors: &[MonitorInfo]) -> String {
    match selected {
        None => "Primary monitor".to_string(),
        Some(idx) => monitors
            .iter()
            .find(|m| m.index == idx)
            .map(monitor_text)
            .unwrap_or_else(|| format!("Monitor {} (unavailable)", idx + 1)),
    }
}

/// Labelled integer drag field clamped to `min..=max`. Returns true when the value changed.
fn int_field(ui: &mut Ui, label: &str, value: &mut i32, min: i32, max: i32) -> bool {
    ui.label(RichText::new(label).color(TEXT).size(14.0));
    let resp = ui.add(egui::DragValue::new(value).range(min..=max).speed(1.0).suffix(" px"));
    resp.changed()
}

fn hotkey_field_mut<'a>(h: &'a mut HotkeySettings, key: &str) -> &'a mut String {
    match key {
        "toggle_crosshair" => &mut h.toggle_crosshair,
        "toggle_overlay" => &mut h.toggle_overlay,
        "next_preset" => &mut h.next_preset,
        "prev_preset" => &mut h.prev_preset,
        _ => &mut h.toggle_auto_profiles,
    }
}

fn hotkey_field<'a>(h: &'a HotkeySettings, key: &str) -> &'a str {
    match key {
        "toggle_crosshair" => &h.toggle_crosshair,
        "toggle_overlay" => &h.toggle_overlay,
        "next_preset" => &h.next_preset,
        "prev_preset" => &h.prev_preset,
        _ => &h.toggle_auto_profiles,
    }
}

fn hotkeys_tab(ui: &mut Ui, state: &mut AppState) {
    let capture_id = state_id(CAPTURE_ID);
    let mut capturing: Option<&'static str> =
        ui.ctx().data(|d| d.get_temp::<Option<&'static str>>(capture_id)).flatten();

    // Finish or cancel an active capture.
    if let Some(key) = capturing {
        let (escape, combo) = ui.input(|i| {
            (i.key_pressed(egui::Key::Escape), capture_hotkey_string(i))
        });
        if escape {
            capturing = None;
        } else if let Some(s) = combo {
            *hotkey_field_mut(&mut state.settings.hotkeys, key) = s;
            state.hotkeys_changed = true;
            state.dirty = true;
            capturing = None;
        }
    }

    widgets::titled_card(
        ui,
        "Global hotkeys",
        Some("Click Change, then press the new key combination. Esc cancels. Hotkeys work while the app is hidden."),
        |ui| {
            for (idx, (key, label, desc)) in HOTKEY_ROWS.iter().enumerate() {
                if idx > 0 {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(*label).size(14.0).color(TEXT));
                        ui.label(RichText::new(*desc).size(12.0).color(TEXT2));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if capturing == Some(*key) {
                            if widgets::secondary_button(ui, "Cancel").clicked() {
                                capturing = None;
                            }
                            ui.label(RichText::new("Press keys...").color(theme::BLUE_LIGHT).strong());
                        } else {
                            if widgets::secondary_button(ui, "Change").clicked() {
                                capturing = Some(*key);
                            }
                            let current = hotkey_field(&state.settings.hotkeys, key).to_string();
                            ui.horizontal(|ui| {
                                for part in current.split('+').filter(|p| !p.is_empty()) {
                                    widgets::key_chip(ui, part.trim());
                                }
                            });
                        }
                    });
                });
                if let Some((_, err)) = state.hotkey_errors.iter().find(|(k, _)| k == key) {
                    ui.label(
                        RichText::new(format!("Could not register: {err}"))
                            .size(12.0)
                            .color(theme::DANGER),
                    );
                }
            }
        },
    );

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if widgets::secondary_button(ui, "Reset hotkeys to defaults").clicked() {
            state.settings.hotkeys = HotkeySettings::default();
            state.hotkeys_changed = true;
            state.dirty = true;
            capturing = None;
        }
    });

    ui.ctx().data_mut(|d| d.insert_temp(capture_id, capturing));
}

fn appearance_tab(ui: &mut Ui, state: &mut AppState) {
    let mut dark = state.settings.preview_dark_background;
    let mut changed = false;
    widgets::titled_card(ui, "Preview", Some("Appearance of the crosshair preview inside the app."), |ui| {
        ui.label(RichText::new("Preview background").size(13.0).color(TEXT2));
        if widgets::segmented(ui, &mut dark, &[(true, "Dark"), (false, "Light")]) {
            state.settings.preview_dark_background = dark;
            changed = true;
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Preview zoom").size(13.0).color(TEXT2));
        let mut zoom = state.settings.preview_zoom;
        if ui
            .add(egui::Slider::new(&mut zoom, 1.0..=8.0).step_by(0.5).suffix("x").max_decimals(1))
            .changed()
        {
            state.settings.preview_zoom = zoom;
            changed = true;
        }
        ui.add_space(12.0);
        let c = state.settings.active_crosshair.clone();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(520.0), 180.0), egui::Sense::hover());
        widgets::preview_box(ui, rect, state.settings.preview_dark_background, &c, state.settings.preview_zoom);
    });
    if changed {
        state.dirty = true;
    }
}

fn detection_tab(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(
        ui,
        "Game detection",
        Some("Automatically apply a game profile's preset while that game is in the foreground."),
        |ui| {
            if widgets::toggle_row(
                ui,
                "Auto profiles",
                Some("Requires a profile for the game on the Game Profiles page."),
                &mut state.settings.auto_profiles,
            ) {
                state.dirty = true;
            }
            ui.add_space(12.0);
            ui.label(RichText::new("Check interval").size(13.0).color(TEXT2));
            let mut ms = state.settings.detection_interval_ms as f32;
            if ui
                .add(egui::Slider::new(&mut ms, 250.0..=5000.0).step_by(50.0).suffix(" ms"))
                .changed()
            {
                state.settings.detection_interval_ms = ms.round() as u32;
                state.dirty = true;
            }
        },
    );
    ui.add_space(4.0);
    widgets::titled_card(ui, "What detection reads", None, |ui| {
        for line in [
            "Only the foreground window's owning process: its executable file path (for example cs2.exe) is compared with your profiles.",
            "Nothing is written to disk, stored in a history, or sent over the network.",
            "Detection never reads game memory, injects code or hooks into other processes.",
            "It runs only while auto profiles are on. Turning them off stops all checks.",
        ] {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new("\u{2022}").color(TEXT2));
                ui.label(RichText::new(line).size(13.0).color(TEXT));
            });
        }
    });
}

fn startup_tab(ui: &mut Ui, state: &mut AppState) {
    widgets::titled_card(
        ui,
        "Startup",
        Some("Start OpenCrosshair when you sign in to Windows."),
        |ui| {
            if widgets::toggle_row(
                ui,
                "Launch at Windows startup",
                Some("Off by default. Registers the app for the current user only; no administrator rights needed."),
                &mut state.settings.launch_at_startup,
            ) {
                state.startup_changed = true;
                state.dirty = true;
            }
        },
    );
}

fn about_tab(ui: &mut Ui) {
    widgets::titled_card(ui, "OpenCrosshair", Some(&format!("Version {}", env!("CARGO_PKG_VERSION"))), |ui| {
        ui.label(
            RichText::new("A crosshair overlay for Windows games. Design a crosshair, save presets, and let game profiles switch them automatically.")
                .size(13.0)
                .color(TEXT),
        );
    });
    ui.add_space(4.0);
    widgets::titled_card(ui, "Privacy", None, |ui| {
        for line in [
            "Runs entirely on this computer. No account, no telemetry, no analytics.",
            "Makes no network requests.",
            "Settings, presets and game profiles are saved as local files in your Windows user profile.",
        ] {
            ui.label(RichText::new(line).size(13.0).color(TEXT2));
        }
    });
}
