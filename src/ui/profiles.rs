//! Game profiles page: per-game crosshair assignments, automatic switching and the default crosshair.
use crate::crosshair::model::Preset;
use crate::profiles::detection::list_running_processes;
use crate::profiles::manager::{add_profile, normalize_exe, remove_profile, update_profile};
use crate::profiles::GameProfile;
use crate::state::AppState;
use crate::ui::presets::{self, muted, FileKind, CAN_BROWSE};
use crate::ui::theme::{self, BLUE_LIGHT, BORDER, DANGER, SUCCESS, SURFACE, TEXT, TEXT2};
use crate::ui::widgets::{danger_button, page_header, paint_crosshair, primary_button, secondary_button, status_pill, toggle_row};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Margin, Rect, RichText, Sense, Stroke, Vec2};

/// Add/edit dialog state. `id` is None while adding.
#[derive(Clone, Default)]
struct ProfileForm {
    id: Option<String>,
    name: String,
    exe: String,
    preset_id: Option<String>,
    enabled: bool,
    error: Option<String>,
    /// Running applications as (file name, full path), shown while the picker is open.
    running: Option<Vec<(String, String)>>,
}

#[derive(Clone, Default)]
struct ProfilesUi {
    form: Option<ProfileForm>,
    delete: Option<String>,
}

enum Action {
    Use(String),
    Edit(String),
    Delete(String),
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("profiles_page_state");
    let mut pu: ProfilesUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();

    ui.horizontal(|ui| {
        ui.vertical(|ui| page_header(ui, "Game profiles", "Assign a crosshair to each game. Profiles apply while the game is in focus."));
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if primary_button(ui, "Add profile").clicked() {
                pu.form = Some(ProfileForm {
                    enabled: true,
                    ..ProfileForm::default()
                });
            }
        });
    });

    switching_card(ui, state);
    ui.add_space(14.0);

    let mut actions = Vec::new();
    profile_list(ui, state, &mut actions);
    for action in actions {
        handle_action(state, &mut pu, action);
    }

    dialogs(ui.ctx(), state, &mut pu);
    ui.data_mut(|d| d.insert_temp(memory_id, pu));
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn switching_card(ui: &mut egui::Ui, state: &mut AppState) {
    card(ui, |ui| {
        let mut auto = state.settings.auto_profiles;
        if toggle_row(
            ui,
            "Switch crosshairs automatically",
            Some("Apply a profile's crosshair when its game comes to the foreground."),
            &mut auto,
        ) {
            state.settings.auto_profiles = auto;
            state.dirty = true;
            let msg = if auto { "Automatic switching is on." } else { "Automatic switching is off." };
            state.toast(msg, false);
        }

        ui.add_space(10.0);
        ui.label(RichText::new("Default crosshair").size(14.0).color(TEXT));
        ui.label(muted("Used when a profile's game leaves focus, and when no profile matches after a game was active."));
        ui.add_space(4.0);
        let current = state.settings.default_profile_preset_id.clone();
        let selected_name = current
            .as_deref()
            .and_then(|id| state.presets.iter().find(|p| p.id == id))
            .map_or("None (keep current crosshair)".to_string(), |p| p.name.clone());
        let mut chosen = current.clone();
        egui::ComboBox::from_id_salt("default_profile_preset")
            .selected_text(selected_name)
            .width(280.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, "None (keep current crosshair)");
                for p in &state.presets {
                    ui.selectable_value(&mut chosen, Some(p.id.clone()), &p.name);
                }
            });
        if chosen != current {
            state.settings.default_profile_preset_id = chosen;
            state.dirty = true;
        }
    });
}

/// Status shown for one profile: reflects the auto-switch setting and whether it is active now.
fn status_for(state: &AppState, p: &GameProfile) -> (&'static str, Color32) {
    if !state.settings.auto_profiles {
        ("Auto-switch off", TEXT2)
    } else if !p.enabled {
        ("Disabled", TEXT2)
    } else if state.active_profile.as_deref() == Some(p.name.as_str()) {
        ("Active", SUCCESS)
    } else {
        ("Watching", BLUE_LIGHT)
    }
}

fn profile_list(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    if state.profiles.is_empty() {
        card(ui, |ui| {
            ui.label(RichText::new("No profiles yet").size(15.0).strong().color(TEXT));
            ui.label(muted("Add a game by its executable to give it its own crosshair."));
        });
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for i in 0..state.profiles.len() {
                let p = state.profiles[i].clone();
                profile_row(ui, state, i, &p, actions);
                ui.add_space(8.0);
            }
        });
}

fn profile_row(ui: &mut egui::Ui, state: &mut AppState, index: usize, p: &GameProfile, actions: &mut Vec<Action>) {
    let preset: Option<Preset> = p
        .preset_id
        .as_deref()
        .and_then(|id| state.presets.iter().find(|pr| pr.id == id).cloned());
    let (status_text, status_color) = status_for(state, p);

    card(ui, |ui| {
        ui.horizontal(|ui| {
            // Preview tile with the assigned crosshair.
            let (tile, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
            ui.painter().rect_filled(tile, CornerRadius::same(8), theme::PREVIEW_DARK);
            match &preset {
                Some(pr) => {
                    let bounds = pr.crosshair.bounding_size() as f32;
                    let zoom = (44.0 / bounds).clamp(0.5, 2.0);
                    paint_crosshair(ui, Rect::from_center_size(tile.center(), Vec2::splat(bounds * zoom)), &pr.crosshair, zoom);
                }
                None => {
                    ui.painter().text(
                        tile.center(),
                        Align2::CENTER_CENTER,
                        "None",
                        egui::FontId::proportional(11.0),
                        TEXT2,
                    );
                }
            }

            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&p.name).size(15.0).strong().color(TEXT));
                ui.label(RichText::new(&p.exe).size(12.0).monospace().color(TEXT2));
                let assigned = match &preset {
                    Some(pr) => format!("Crosshair: {}", pr.name),
                    None if p.preset_id.is_some() => "Crosshair: preset was deleted".to_string(),
                    None => "No crosshair assigned".to_string(),
                };
                ui.label(RichText::new(assigned).size(12.0).color(TEXT2));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if danger_button(ui, "Delete").clicked() {
                    actions.push(Action::Delete(p.id.clone()));
                }
                if secondary_button(ui, "Edit").clicked() {
                    actions.push(Action::Edit(p.id.clone()));
                }
                if secondary_button(ui, "Use now").clicked() {
                    actions.push(Action::Use(p.id.clone()));
                }
                let mut enabled = p.enabled;
                if ui.checkbox(&mut enabled, "Enabled").changed() {
                    state.profiles[index].enabled = enabled;
                    state.dirty = true;
                }
                status_pill(ui, status_text, status_color);
            });
        });
    });
}

fn handle_action(state: &mut AppState, pu: &mut ProfilesUi, action: Action) {
    match action {
        Action::Use(id) => {
            let Some(p) = state.profiles.iter().find(|p| p.id == id).cloned() else { return };
            let Some(preset_id) = p.preset_id.clone() else {
                state.toast(format!("\"{}\" has no crosshair assigned. Edit the profile to choose one.", p.name), true);
                return;
            };
            if !state.presets.iter().any(|pr| pr.id == preset_id) {
                state.toast(format!("The crosshair for \"{}\" no longer exists. Edit the profile.", p.name), true);
                return;
            }
            state.apply_preset(&preset_id);
            state.active_profile = Some(p.name.clone());
            state.toast(format!("Using the crosshair for \"{}\".", p.name), false);
        }
        Action::Edit(id) => {
            if let Some(p) = state.profiles.iter().find(|p| p.id == id) {
                pu.form = Some(ProfileForm {
                    id: Some(p.id.clone()),
                    name: p.name.clone(),
                    exe: p.exe.clone(),
                    preset_id: p.preset_id.clone(),
                    enabled: p.enabled,
                    error: None,
                    running: None,
                });
            }
        }
        Action::Delete(id) => pu.delete = Some(id),
    }
}

fn dialogs(ctx: &egui::Context, state: &mut AppState, pu: &mut ProfilesUi) {
    form_dialog(ctx, state, pu);
    delete_dialog(ctx, state, pu);
}

/// File stem of an executable name, used as the default profile name ("cs2.exe" -> "cs2").
fn stem_of(exe: &str) -> String {
    exe.strip_suffix(".exe").unwrap_or(exe).to_string()
}

fn form_dialog(ctx: &egui::Context, state: &mut AppState, pu: &mut ProfilesUi) {
    let Some(mut form) = pu.form.take() else { return };
    let title = if form.id.is_some() { "Edit profile" } else { "Add profile" };
    let presets_list: Vec<(String, String)> = state.presets.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    let mut open = true;
    let mut save = false;
    let mut cancel = false;

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(muted("Name"));
            ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text("e.g. Counter-Strike 2")
                    .desired_width(440.0),
            );

            ui.add_space(6.0);
            ui.label(muted("Executable"));
            ui.horizontal(|ui| {
                let width = if CAN_BROWSE { 330.0 } else { 440.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut form.exe)
                        .hint_text("cs2.exe")
                        .desired_width(width),
                );
                if CAN_BROWSE && secondary_button(ui, "Browse...").clicked() {
                    if let Some(path) = presets::pick_path(FileKind::Exe, false, "") {
                        let exe = normalize_exe(&path.display().to_string());
                        if form.name.trim().is_empty() {
                            form.name = stem_of(&exe);
                        }
                        form.exe = exe;
                        form.error = None;
                    }
                }
            });
            ui.add_space(4.0);
            let picker_open = form.running.is_some();
            if secondary_button(ui, if picker_open { "Hide running apps" } else { "Pick from running apps" }).clicked() {
                form.running = if picker_open {
                    None
                } else {
                    let mut list: Vec<(String, String)> = list_running_processes()
                        .into_iter()
                        .map(|r| (r.name, r.path))
                        .collect();
                    list.sort();
                    list.dedup_by(|a, b| a.0 == b.0);
                    Some(list)
                };
            }
            if let Some(list) = form.running.clone() {
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if list.is_empty() {
                            ui.label(muted("No running applications found."));
                        }
                        for (name, path) in &list {
                            if ui.selectable_label(false, name).on_hover_text(path).clicked() {
                                if form.name.trim().is_empty() {
                                    form.name = stem_of(name);
                                }
                                form.exe = name.clone();
                                form.running = None;
                                form.error = None;
                                break;
                            }
                        }
                    });
            }

            ui.add_space(6.0);
            ui.label(muted("Crosshair"));
            let selected = form
                .preset_id
                .as_ref()
                .and_then(|id| presets_list.iter().find(|(pid, _)| pid == id))
                .map_or("None".to_string(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt("profile_form_preset")
                .selected_text(selected)
                .width(440.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.preset_id, None, "None");
                    for (pid, name) in &presets_list {
                        ui.selectable_value(&mut form.preset_id, Some(pid.clone()), name);
                    }
                });

            if let Some(err) = &form.error {
                ui.add_space(6.0);
                ui.label(RichText::new(err).size(12.0).color(DANGER));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Save").clicked() {
                    save = true;
                }
                if secondary_button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if save {
        let preset_id = form.preset_id.clone();
        let result = match form.id.clone() {
            None => add_profile(&mut state.profiles, &form.name, &form.exe, preset_id).map(|_| "Profile added."),
            Some(id) => update_profile(&mut state.profiles, &id, &form.name, &form.exe, preset_id, form.enabled)
                .map(|()| "Profile saved."),
        };
        match result {
            Ok(msg) => {
                state.dirty = true;
                state.toast(msg, false);
                return;
            }
            Err(e) => form.error = Some(e.to_string()),
        }
    }
    if cancel || !open {
        return;
    }
    pu.form = Some(form);
}

fn delete_dialog(ctx: &egui::Context, state: &mut AppState, pu: &mut ProfilesUi) {
    let Some(id) = pu.delete.clone() else { return };
    let name = state.profiles.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Window::new("Delete profile?")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.label(format!("\"{name}\" will be removed. Its game will no longer switch crosshairs."));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if danger_button(ui, "Delete").clicked() {
                    confirmed = true;
                }
                if secondary_button(ui, "Cancel").clicked() {
                    cancelled = true;
                }
            });
        });
    if confirmed {
        match remove_profile(&mut state.profiles, &id) {
            Ok(removed) => {
                if state.active_profile.as_deref() == Some(removed.name.as_str()) {
                    state.active_profile = None;
                }
                state.dirty = true;
                state.toast(format!("Deleted profile \"{}\".", removed.name), false);
            }
            Err(e) => state.toast(e.to_string(), true),
        }
    }
    if confirmed || cancelled || !open {
        pu.delete = None;
    }
}
