//! Game profiles page. A list of profiles on the left and the detail/edit form of the selected
//! profile on the right. Above them: a card with the automatic switching toggle and the default
//! crosshair. Adding a profile opens the same form in the right-hand panel.
use crate::crosshair::model::Preset;
use crate::profiles::GameProfile;
use crate::profiles::detection::list_running_processes;
use crate::profiles::manager::{add_profile, normalize_exe, remove_profile, update_profile};
use crate::state::AppState;
use crate::ui::presets::{self, CAN_BROWSE, FileKind};
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{self, PreviewScene};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, UiBuilder, Vec2,
};

/// Width of the profile list column.
const LIST_W: f32 = 300.0;
/// Space between the list and the detail column.
const COLUMN_GAP: f32 = 16.0;
/// Height the profile list may use before it scrolls.
const LIST_MAX_H: f32 = 560.0;
/// Height of one profile row.
const ROW_H: f32 = 88.0;
const ROW_PAD_X: f32 = 12.0;
const ROW_PAD_Y: f32 = 10.0;
/// Square preview of a crosshair inside a row.
const TILE: f32 = 40.0;
/// Square preview of the selected preset in the detail form.
const PREVIEW_SIZE: f32 = 96.0;

/// Detail/edit form. `id` is None while adding a new profile.
#[derive(Clone, Default)]
struct ProfileForm {
    id: Option<String>,
    name: String,
    exe: String,
    preset_id: Option<String>,
    error: Option<String>,
    /// Running applications as (file name, full path), shown while the picker is open.
    running: Option<Vec<(String, String)>>,
}

impl ProfileForm {
    fn for_profile(p: &GameProfile) -> Self {
        Self {
            id: Some(p.id.clone()),
            name: p.name.clone(),
            exe: p.exe.clone(),
            preset_id: p.preset_id.clone(),
            ..Self::default()
        }
    }
}

/// Page memory kept between frames.
#[derive(Clone, Default)]
struct ProfilesUi {
    /// Form of the selected profile, or of a new profile. None when nothing is open.
    form: Option<ProfileForm>,
    /// Id of the profile whose deletion awaits confirmation.
    delete: Option<String>,
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("profiles_page_state");
    let mut pu: ProfilesUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            widgets::page_header(
                ui,
                "Game profiles",
                "Assign a crosshair to each game. Profiles apply while the game is in focus.",
            );
        });
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if widgets::primary_button(ui, "Add profile").clicked() {
                pu.form = Some(ProfileForm::default());
                pu.delete = None;
            }
        });
    });

    switching_card(ui, state);
    ui.add_space(14.0);

    let selected = pu.form.as_ref().and_then(|f| f.id.clone());
    let mut clicked: Option<String> = None;
    ui.horizontal_top(|ui| {
        let spacing = ui.spacing().item_spacing.x;
        let detail_w = (ui.available_width() - LIST_W - COLUMN_GAP - 2.0 * spacing).max(0.0);
        ui.vertical(|ui| {
            ui.set_width(LIST_W);
            clicked = profile_list(ui, state, selected.as_deref());
        });
        ui.add_space(COLUMN_GAP);
        ui.vertical(|ui| {
            ui.set_width(detail_w);
            detail_panel(ui, state, &mut pu);
        });
    });

    if let Some(id) = clicked
        && pu.form.as_ref().and_then(|f| f.id.as_deref()) != Some(id.as_str())
        && let Some(profile) = state.profiles.iter().find(|p| p.id == id)
    {
        pu.form = Some(ProfileForm::for_profile(profile));
        pu.delete = None;
    }

    ui.data_mut(|d| d.insert_temp(memory_id, pu));
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(text).size(13.0).color(p.text_muted));
}

fn switching_card(ui: &mut egui::Ui, state: &mut AppState) {
    let p = theme::palette(ui.ctx());
    widgets::card(ui, |ui| {
        let mut auto = state.settings.auto_profiles;
        if widgets::toggle_row(
            ui,
            "Switch crosshairs automatically",
            Some("Apply a profile's crosshair when its game comes to the foreground."),
            &mut auto,
        ) {
            state.settings.auto_profiles = auto;
            state.dirty = true;
            let msg = if auto {
                "Automatic switching is on."
            } else {
                "Automatic switching is off."
            };
            state.toast(msg, false);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("Default crosshair").size(14.0).color(p.text));
        ui.label(
            RichText::new(
                "Used when a profile's game leaves focus, and when no profile matches after a game was active.",
            )
            .size(12.0)
            .color(p.text_muted),
        );
        ui.add_space(6.0);
        let current = state.settings.default_profile_preset_id.clone();
        let selected_name = current
            .as_deref()
            .and_then(|id| state.presets.iter().find(|pr| pr.id == id))
            .map_or("None (keep current crosshair)".to_string(), |pr| pr.name.clone());
        let mut chosen = current.clone();
        egui::ComboBox::from_id_salt("default_profile_preset")
            .selected_text(selected_name)
            .width(280.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, "None (keep current crosshair)");
                for pr in &state.presets {
                    ui.selectable_value(&mut chosen, Some(pr.id.clone()), &pr.name);
                }
            });
        if chosen != current {
            state.settings.default_profile_preset_id = chosen;
            state.dirty = true;
        }
    });
}

/// Status of one profile: whether auto-switching is on and whether this profile is active now.
fn status_for(state: &AppState, profile: &GameProfile, p: &Palette) -> (&'static str, Color32) {
    if !state.settings.auto_profiles {
        ("Auto-switch off", p.text_muted)
    } else if !profile.enabled {
        ("Disabled", p.text_muted)
    } else if state.active_profile.as_deref() == Some(profile.name.as_str()) {
        ("Active", p.success)
    } else {
        ("Watching", p.primary)
    }
}

/// Left column. Returns the id of a row that was clicked this frame.
fn profile_list(ui: &mut egui::Ui, state: &mut AppState, selected: Option<&str>) -> Option<String> {
    if state.profiles.is_empty() {
        widgets::empty_state(
            ui,
            "No profiles yet",
            "Add a game by its executable to give it its own crosshair.",
        );
        return None;
    }
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .max_height(LIST_MAX_H)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for index in 0..state.profiles.len() {
                let profile = state.profiles[index].clone();
                let is_selected = selected == Some(profile.id.as_str());
                if profile_row(ui, state, index, &profile, is_selected) {
                    clicked = Some(profile.id.clone());
                }
                ui.add_space(8.0);
            }
        });
    clicked
}

/// One profile row fixed at `ROW_H`. The row is registered before its contents, so the enable
/// switch takes clicks on its own area and the rest of the row selects the profile. Returns true
/// when the row body was clicked.
fn profile_row(ui: &mut egui::Ui, state: &mut AppState, index: usize, profile: &GameProfile, selected: bool) -> bool {
    let p = theme::palette(ui.ctx());
    let preset: Option<Preset> = profile
        .preset_id
        .as_deref()
        .and_then(|id| state.presets.iter().find(|pr| pr.id == id))
        .cloned();
    let (status_text, status_color) = status_for(state, profile, p);
    let assigned = match &preset {
        Some(pr) => format!("Crosshair: {}", pr.name),
        None if profile.preset_id.is_some() => "Crosshair: preset was deleted".to_string(),
        None => "No crosshair assigned".to_string(),
    };

    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), ROW_H));
    let resp = ui
        .interact(
            rect,
            ui.scope_id().with(("oc_profile_row", profile.id.as_str())),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let (fill, border) = if selected {
        (p.accent_soft, p.accent)
    } else if resp.hovered() {
        (p.surface_alt, p.border)
    } else {
        (p.surface, p.border)
    };
    ui.painter().rect_filled(rect, 10, fill);
    ui.painter()
        .rect_stroke(rect, 10, Stroke::new(1.0, border), StrokeKind::Inside);

    let mut enabled = profile.enabled;
    let mut toggled = false;
    ui.scope_builder(
        UiBuilder::new().max_rect(rect.shrink2(Vec2::new(ROW_PAD_X, ROW_PAD_Y))),
        |ui| {
            ui.set_min_height(ROW_H - 2.0 * ROW_PAD_Y);
            ui.horizontal_top(|ui| {
                let (tile, _) = ui.allocate_exact_size(Vec2::splat(TILE), Sense::hover());
                match &preset {
                    Some(pr) => widgets::preview_scene_box(ui, tile, PreviewScene::Dark, &pr.crosshair, 2.0),
                    None => {
                        ui.painter().rect_filled(tile, 6, p.surface_alt);
                        ui.painter()
                            .rect_stroke(tile, 6, Stroke::new(1.0, p.border), StrokeKind::Inside);
                        ui.painter().text(
                            tile.center(),
                            Align2::CENTER_CENTER,
                            "None",
                            FontId::proportional(11.0),
                            p.text_muted,
                        );
                    }
                }
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&profile.name).size(14.0).strong().color(p.text)).truncate(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            toggled = widgets::toggle_switch(ui, &mut enabled).changed();
                        });
                    });
                    ui.label(RichText::new(&profile.exe).size(12.0).monospace().color(p.text_muted));
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(&assigned).size(12.0).color(p.text_muted)).truncate());
                        widgets::status_pill(ui, status_text, status_color);
                    });
                });
            });
        },
    );

    if toggled {
        state.profiles[index].enabled = enabled;
        state.dirty = true;
    }
    resp.clicked() && !toggled
}

/// Right column: the form of the selected or new profile, with its actions.
fn detail_panel(ui: &mut egui::Ui, state: &mut AppState, pu: &mut ProfilesUi) {
    let p = theme::palette(ui.ctx());
    let Some(mut form) = pu.form.take() else {
        widgets::card(ui, |ui| {
            ui.label(widgets::heading_text("No profile selected", 17.0));
            ui.add_space(4.0);
            ui.label(
                RichText::new("Select a profile to edit it, or add one for a game.")
                    .size(13.0)
                    .color(p.text_muted),
            );
        });
        return;
    };

    let editing_id = form.id.clone();
    let saved = editing_id
        .as_deref()
        .and_then(|id| state.profiles.iter().find(|pr| pr.id == id))
        .map(|pr| (pr.name.clone(), pr.preset_id.clone()));
    let confirming = editing_id.is_some() && pu.delete == editing_id;

    let mut closed = false;
    let mut save = false;
    let mut use_now = false;
    let mut ask_delete = false;
    let mut confirm_delete = false;
    let mut keep = false;

    widgets::card(ui, |ui| {
        let title = if editing_id.is_some() {
            "Edit profile"
        } else {
            "New profile"
        };
        ui.label(widgets::heading_text(title, 17.0));
        ui.add_space(10.0);

        field_label(ui, "Name");
        let width = ui.available_width();
        ui.add(
            egui::TextEdit::singleline(&mut form.name)
                .hint_text("e.g. Counter-Strike 2")
                .desired_width(width),
        );
        ui.add_space(8.0);

        field_label(ui, "Executable");
        ui.horizontal(|ui| {
            let browse_w = if CAN_BROWSE { 110.0 } else { 0.0 };
            let width = (ui.available_width() - browse_w).max(120.0);
            ui.add(
                egui::TextEdit::singleline(&mut form.exe)
                    .hint_text("cs2.exe")
                    .desired_width(width),
            );
            if CAN_BROWSE
                && widgets::secondary_button(ui, "Browse...").clicked()
                && let Some(path) = presets::pick_path(FileKind::Exe, false, "")
            {
                let exe = normalize_exe(&path.display().to_string());
                if form.name.trim().is_empty() {
                    form.name = stem_of(&exe);
                }
                form.exe = exe;
                form.error = None;
            }
        });
        ui.add_space(6.0);

        let picker_open = form.running.is_some();
        let picker_label = if picker_open {
            "Hide running apps"
        } else {
            "Pick from running apps"
        };
        if widgets::secondary_button(ui, picker_label).clicked() {
            form.running = if picker_open { None } else { Some(running_apps()) };
        }
        if let Some(list) = form.running.clone() {
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if list.is_empty() {
                        ui.label(
                            RichText::new("No running applications found.")
                                .size(12.0)
                                .color(p.text_muted),
                        );
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
        ui.add_space(8.0);

        field_label(ui, "Crosshair");
        let selected_name = form
            .preset_id
            .as_deref()
            .and_then(|id| state.presets.iter().find(|pr| pr.id == id))
            .map_or("None".to_string(), |pr| pr.name.clone());
        ui.horizontal_top(|ui| {
            egui::ComboBox::from_id_salt("profile_form_preset")
                .selected_text(selected_name)
                .width(240.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.preset_id, None, "None");
                    for pr in &state.presets {
                        ui.selectable_value(&mut form.preset_id, Some(pr.id.clone()), &pr.name);
                    }
                });
            let chosen = form
                .preset_id
                .as_deref()
                .and_then(|id| state.presets.iter().find(|pr| pr.id == id));
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(PREVIEW_SIZE), Sense::hover());
            match chosen {
                Some(pr) => widgets::preview_scene_box(ui, rect, PreviewScene::Dark, &pr.crosshair, 4.0),
                None => {
                    ui.painter().rect_filled(rect, 6, p.surface_alt);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "No crosshair",
                        FontId::proportional(12.0),
                        p.text_muted,
                    );
                }
            }
        });

        if let Some(err) = &form.error {
            ui.add_space(6.0);
            ui.label(RichText::new(err).size(12.0).color(p.danger));
        }
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if widgets::primary_button(ui, "Save").clicked() {
                save = true;
            }
            if widgets::secondary_button(ui, "Cancel").clicked() {
                closed = true;
            }
            if editing_id.is_some() {
                let has_preset = saved.as_ref().is_some_and(|(_, preset)| preset.is_some());
                ui.add_enabled_ui(has_preset, |ui| {
                    if widgets::secondary_button(ui, "Use now").clicked() {
                        use_now = true;
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !confirming && widgets::danger_button(ui, "Delete").clicked() {
                        ask_delete = true;
                    }
                });
            }
        });

        if confirming {
            ui.add_space(12.0);
            let name = saved.as_ref().map(|(n, _)| n.as_str()).unwrap_or_default();
            ui.label(
                RichText::new(format!(
                    "\"{name}\" will be removed. Its game will no longer switch crosshairs."
                ))
                .size(13.0)
                .color(p.text),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, "Delete profile").clicked() {
                    confirm_delete = true;
                }
                if widgets::secondary_button(ui, "Keep profile").clicked() {
                    keep = true;
                }
            });
        }
    });

    if ask_delete {
        pu.delete = editing_id.clone();
    }
    if keep {
        pu.delete = None;
    }
    if use_now && let Some(id) = editing_id.as_deref() {
        use_profile(state, id);
    }
    if confirm_delete && let Some(id) = editing_id.as_deref() {
        match remove_profile(&mut state.profiles, id) {
            Ok(removed) => {
                if state.active_profile.as_deref() == Some(removed.name.as_str()) {
                    state.active_profile = None;
                }
                state.dirty = true;
                state.toast(format!("Deleted profile \"{}\".", removed.name), false);
                pu.delete = None;
                closed = true;
            }
            Err(e) => state.toast(e.to_string(), true),
        }
    }
    if save {
        let before = editing_id
            .as_ref()
            .and_then(|id| state.profiles.iter().find(|pr| &pr.id == id))
            .map(|pr| (pr.name.clone(), pr.enabled));
        let result = match editing_id.as_deref() {
            None => add_profile(&mut state.profiles, &form.name, &form.exe, form.preset_id.clone())
                .map(|id| (id, "Profile added.")),
            Some(id) => {
                let enabled = before.as_ref().is_none_or(|(_, enabled)| *enabled);
                update_profile(
                    &mut state.profiles,
                    id,
                    &form.name,
                    &form.exe,
                    form.preset_id.clone(),
                    enabled,
                )
                .map(|()| (id.to_string(), "Profile saved."))
            }
        };
        match result {
            Ok((id, msg)) => {
                if let Some((old_name, _)) = &before
                    && state.active_profile.as_deref() == Some(old_name.as_str())
                    && let Some(renamed) = state.profiles.iter().find(|pr| pr.id == id)
                {
                    state.active_profile = Some(renamed.name.clone());
                }
                state.dirty = true;
                state.toast(msg, false);
                // Keep the saved profile open so its normalized values are visible.
                if let Some(saved_profile) = state.profiles.iter().find(|pr| pr.id == id) {
                    form = ProfileForm::for_profile(saved_profile);
                }
            }
            Err(e) => form.error = Some(e.to_string()),
        }
    }

    if closed {
        pu.delete = None;
    } else {
        pu.form = Some(form);
    }
}

/// Applies the crosshair of a saved profile and marks it as the active profile.
fn use_profile(state: &mut AppState, id: &str) {
    let Some(profile) = state.profiles.iter().find(|pr| pr.id == id).cloned() else {
        return;
    };
    let Some(preset_id) = profile.preset_id.clone() else {
        state.toast(
            format!(
                "\"{}\" has no crosshair assigned. Edit the profile to choose one.",
                profile.name
            ),
            true,
        );
        return;
    };
    if !state.presets.iter().any(|pr| pr.id == preset_id) {
        state.toast(
            format!(
                "The crosshair for \"{}\" no longer exists. Edit the profile.",
                profile.name
            ),
            true,
        );
        return;
    }
    state.apply_preset(&preset_id);
    state.active_profile = Some(profile.name.clone());
    state.toast(format!("Using the crosshair for \"{}\".", profile.name), false);
}

/// Running applications as sorted, de-duplicated (file name, full path) pairs.
fn running_apps() -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> = list_running_processes().into_iter().map(|r| (r.name, r.path)).collect();
    list.sort();
    list.dedup_by(|a, b| a.0 == b.0);
    list
}

/// File stem of an executable name, used as the default profile name ("cs2.exe" -> "cs2").
fn stem_of(exe: &str) -> String {
    exe.strip_suffix(".exe").unwrap_or(exe).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_name_drops_exe_suffix() {
        assert_eq!(stem_of("cs2.exe"), "cs2");
        assert_eq!(stem_of("game"), "game");
    }

    #[test]
    fn form_for_profile_copies_saved_values() {
        let profile = GameProfile {
            id: "p1".into(),
            name: "CS2".into(),
            exe: "cs2.exe".into(),
            preset_id: Some("preset-a".into()),
            enabled: false,
        };
        let form = ProfileForm::for_profile(&profile);
        assert_eq!(form.id.as_deref(), Some("p1"));
        assert_eq!(form.name, "CS2");
        assert_eq!(form.exe, "cs2.exe");
        assert_eq!(form.preset_id.as_deref(), Some("preset-a"));
        assert!(form.error.is_none());
        assert!(form.running.is_none());
    }
}
