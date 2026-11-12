//! Crosshair editor page: grouped property panel, live preview and top actions.
use crate::crosshair::model::{Crosshair, Preset, Rgba, Shape, limits};
use crate::crosshair::validation;
use crate::state::AppState;
use crate::storage::import_export::{export_crosshair, parse_import};
use crate::ui::presets::{self, FileKind, PathPrompt, PromptOutcome, muted};
use crate::ui::theme::{self, BLUE_LIGHT, BORDER, DANGER, SURFACE, TEXT, TEXT2};
use crate::ui::widgets::{danger_button, paint_crosshair, primary_button, secondary_button};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};

const PANEL_WIDTH: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PreviewBg {
    Dark,
    Light,
    Checker,
    Gray(u8),
    Game,
}

#[derive(Clone, Default)]
struct HexField {
    text: String,
    /// Colour the text currently mirrors. When the colour changes elsewhere, the text is refreshed.
    last: Option<Rgba>,
    invalid: bool,
}

#[derive(Clone)]
struct TestSession {
    prev_enabled: bool,
    prev_visible: bool,
    prev_active: Crosshair,
}

#[derive(Clone, Default)]
struct SaveForm {
    name: String,
    category: String,
    description: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Purpose {
    Import,
    Export,
}

/// Per-viewer UI memory (not persisted): background, pan, hex text, open dialogs.
#[derive(Clone, Default)]
struct EditorUi {
    bg: Option<PreviewBg>,
    pan: Vec2,
    guides: Option<bool>,
    live: Option<bool>,
    main_hex: HexField,
    outline_hex: HexField,
    confirm_reset: bool,
    save_form: Option<SaveForm>,
    prompt: Option<(Purpose, PathPrompt)>,
    test: Option<TestSession>,
}

/// Preset whose crosshair equals `c`, used to keep `active_preset_id` honest.
fn matching_preset_id(presets: &[Preset], c: &Crosshair) -> Option<String> {
    presets.iter().find(|p| p.crosshair == *c).map(|p| p.id.clone())
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("editor_page_state");
    let mut ui_state: EditorUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();
    let before = state.editing.clone();

    ui.vertical(|ui| {
        top_bar(ui, state, &mut ui_state);
        ui.add_space(10.0);
        ui.horizontal_top(|ui| {
            let height = ui.available_height();
            ui.allocate_ui_with_layout(Vec2::new(PANEL_WIDTH, height), Layout::top_down(Align::Min), |ui| {
                property_panel(ui, state, &mut ui_state);
            });
            ui.add_space(12.0);
            ui.vertical(|ui| preview_area(ui, state, &mut ui_state));
        });
    });

    dialogs(ui.ctx(), state, &mut ui_state);

    // Clamp after edits, then push changes to the overlay when the editor drives it.
    validation::clamp(&mut state.editing);
    if state.editing != before {
        let live_on = ui_state.live.unwrap_or(state.settings.overlay_enabled);
        let driving_overlay = ui_state.test.is_some() || (live_on && state.settings.overlay_enabled);
        if driving_overlay {
            state.settings.active_crosshair = state.editing.clone();
            state.settings.active_preset_id = matching_preset_id(&state.presets, &state.editing);
            state.push_overlay();
            state.dirty = true;
        }
    }

    ui.data_mut(|d| d.insert_temp(memory_id, ui_state));
}

// ---------------------------------------------------------------- top bar

fn top_bar(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Editor").size(20.0).strong().color(TEXT));
        ui.add_space(12.0);
        if secondary_button(ui, "Reset").clicked() {
            ui_state.confirm_reset = true;
        }
        if secondary_button(ui, "Save as preset").clicked() {
            ui_state.save_form = Some(SaveForm {
                category: "Custom".into(),
                ..SaveForm::default()
            });
        }
        if primary_button(ui, "Apply").clicked() {
            apply(state, ui_state);
        }
        if ui_state.test.is_some() {
            if danger_button(ui, "Stop test overlay").clicked() {
                stop_test(state, ui_state);
            }
            ui.label(RichText::new("Test overlay running").size(12.0).color(BLUE_LIGHT));
        } else if secondary_button(ui, "Test overlay").clicked() {
            start_test(state, ui_state);
        }
        ui.add_space(8.0);
        if secondary_button(ui, "Import crosshair").clicked() {
            ui_state.prompt = Some((
                Purpose::Import,
                PathPrompt::new("Import crosshair", false, FileKind::Json, "", String::new()),
            ));
        }
        if secondary_button(ui, "Export crosshair").clicked() {
            let preset = Preset {
                name: "Crosshair".into(),
                crosshair: state.editing.clone(),
                ..Preset::default()
            };
            ui_state.prompt = Some((
                Purpose::Export,
                PathPrompt::new(
                    "Export crosshair",
                    true,
                    FileKind::Json,
                    "crosshair.json",
                    export_crosshair(&preset),
                ),
            ));
        }
        ui.add_space(8.0);
        let mut live = ui_state.live.unwrap_or(state.settings.overlay_enabled);
        if ui.checkbox(&mut live, "Live apply").changed() {
            ui_state.live = Some(live);
        }
    });
}

fn apply(state: &mut AppState, ui_state: &mut EditorUi) {
    if let Some(test) = ui_state.test.take() {
        // Applying ends the temporary test: keep the new crosshair, restore the overlay flags.
        state.settings.overlay_enabled = test.prev_enabled;
        state.settings.crosshair_visible = test.prev_visible;
    }
    state.settings.active_crosshair = state.editing.clone();
    state.settings.active_preset_id = matching_preset_id(&state.presets, &state.editing);
    state.dirty = true;
    state.push_overlay();
    state.toast("Crosshair applied to the overlay.", false);
}

fn start_test(state: &mut AppState, ui_state: &mut EditorUi) {
    let session = TestSession {
        prev_enabled: state.settings.overlay_enabled,
        prev_visible: state.settings.crosshair_visible,
        prev_active: state.settings.active_crosshair.clone(),
    };
    state.settings.overlay_enabled = true;
    state.settings.crosshair_visible = true;
    state.settings.active_crosshair = state.editing.clone();
    state.dirty = true;
    state.push_overlay();
    ui_state.test = Some(session);
    state.toast(
        "Test overlay is showing the editor crosshair. Stop the test to restore the previous overlay.",
        false,
    );
}

fn stop_test(state: &mut AppState, ui_state: &mut EditorUi) {
    if let Some(test) = ui_state.test.take() {
        state.settings.overlay_enabled = test.prev_enabled;
        state.settings.crosshair_visible = test.prev_visible;
        state.settings.active_crosshair = test.prev_active;
        state.dirty = true;
        state.push_overlay();
        state.toast("Test overlay stopped. The previous overlay state is restored.", false);
    }
}

// ---------------------------------------------------------------- property panel

fn property_panel(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let c = &mut state.editing;
                section(ui, "Shape", true, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Type").color(TEXT2));
                        egui::ComboBox::from_id_salt("editor_shape")
                            .selected_text(c.shape.label())
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for shape in Shape::ALL {
                                    ui.selectable_value(&mut c.shape, shape, shape.label());
                                }
                            });
                    });
                });

                section(ui, "Geometry", true, |ui| {
                    slider(
                        ui,
                        "Horizontal length",
                        &mut c.length_h,
                        0.0..=limits::MAX_LENGTH,
                        " px",
                    );
                    slider(ui, "Vertical length", &mut c.length_v, 0.0..=limits::MAX_LENGTH, " px");
                    slider(ui, "Thickness", &mut c.thickness, 0.0..=limits::MAX_THICKNESS, " px");
                    slider(ui, "Gap", &mut c.gap, 0.0..=limits::MAX_GAP, " px");
                    slider(
                        ui,
                        "Circle radius",
                        &mut c.circle_radius,
                        0.0..=limits::MAX_RADIUS,
                        " px",
                    );
                    slider(ui, "Dot size", &mut c.dot_size, 0.0..=limits::MAX_DOT, " px");
                    slider(ui, "Rotation", &mut c.rotation, 0.0..=360.0, "°");
                    slider(
                        ui,
                        "Offset X",
                        &mut c.offset_x,
                        -limits::MAX_OFFSET..=limits::MAX_OFFSET,
                        " px",
                    );
                    slider(
                        ui,
                        "Offset Y",
                        &mut c.offset_y,
                        -limits::MAX_OFFSET..=limits::MAX_OFFSET,
                        " px",
                    );
                });

                section(ui, "Elements", true, |ui| {
                    ui.checkbox(&mut c.show_lines, "Lines");
                    ui.checkbox(&mut c.show_dot, "Center dot");
                    ui.checkbox(&mut c.show_circle, "Circle");
                });

                section(ui, "Appearance", true, |ui| {
                    color_editor(ui, "Color", &mut c.color, &mut ui_state.main_hex);
                    slider(ui, "Opacity", &mut c.opacity, 0.0..=1.0, "");

                    ui.add_space(6.0);
                    ui.checkbox(&mut c.outline, "Outline");
                    ui.add_enabled_ui(c.outline, |ui| {
                        slider(
                            ui,
                            "Outline thickness",
                            &mut c.outline_thickness,
                            0.0..=limits::MAX_OUTLINE,
                            " px",
                        );
                        color_editor(ui, "Outline color", &mut c.outline_color, &mut ui_state.outline_hex);
                    });

                    ui.add_space(6.0);
                    ui.checkbox(&mut c.shadow, "Shadow");
                    ui.add_enabled_ui(c.shadow, |ui| {
                        slider(
                            ui,
                            "Shadow offset",
                            &mut c.shadow_offset,
                            0.0..=validation::MAX_SHADOW_OFFSET,
                            " px",
                        );
                        slider(ui, "Shadow opacity", &mut c.shadow_opacity, 0.0..=1.0, "");
                    });
                });
            });
        });
}

fn section(ui: &mut egui::Ui, title: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong().color(TEXT))
        .id_salt(title)
        .default_open(open)
        .show(ui, |ui| {
            ui.add_space(4.0);
            add(ui);
            ui.add_space(8.0);
        });
}

fn slider(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([118.0, 18.0], egui::Label::new(RichText::new(label).color(TEXT2)));
        ui.add(
            egui::Slider::new(value, range)
                .suffix(suffix)
                .fixed_decimals(if suffix == " px" { 1 } else { 2 }),
        );
    });
}

/// RGB sliders plus a validated HEX text field for one colour.
fn color_editor(ui: &mut egui::Ui, label: &str, color: &mut Rgba, hex: &mut HexField) {
    ui.label(RichText::new(label).color(TEXT2).size(12.0));
    ui.horizontal(|ui| {
        let (swatch, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
        ui.painter().rect_filled(
            swatch,
            CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
        );
        ui.painter().rect_stroke(
            swatch,
            CornerRadius::same(6),
            Stroke::new(1.0, BORDER),
            StrokeKind::Inside,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().slider_width = 150.0;
            ui.add(egui::Slider::new(&mut color.r, 0..=255).text("R"));
            ui.add(egui::Slider::new(&mut color.g, 0..=255).text("G"));
            ui.add(egui::Slider::new(&mut color.b, 0..=255).text("B"));
        });
    });

    // Refresh the text when the colour changed from the sliders (or elsewhere).
    if hex.last != Some(*color) {
        hex.text = color.to_hex();
        hex.last = Some(*color);
        hex.invalid = false;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("HEX").color(TEXT2).size(12.0));
        let resp = ui.add(
            egui::TextEdit::singleline(&mut hex.text)
                .hint_text("#RRGGBB")
                .desired_width(110.0),
        );
        if resp.changed() {
            match Rgba::from_hex(&hex.text) {
                Some(parsed) => {
                    let digits = hex.text.trim().trim_start_matches('#').len();
                    let alpha = if digits == 8 { parsed.a } else { color.a };
                    *color = Rgba::new(parsed.r, parsed.g, parsed.b, alpha);
                    hex.last = Some(*color);
                    hex.invalid = false;
                }
                None => hex.invalid = true,
            }
        }
        if hex.invalid {
            ui.label(RichText::new("Use #RRGGBB").size(12.0).color(DANGER));
        }
    });
}

// ---------------------------------------------------------------- preview

fn preview_area(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    let mut bg = ui_state.bg.unwrap_or(if state.settings.preview_dark_background {
        PreviewBg::Dark
    } else {
        PreviewBg::Light
    });

    ui.horizontal_wrapped(|ui| {
        ui.label(muted("Background"));
        let choices = [
            ("Dark", PreviewBg::Dark),
            ("Light", PreviewBg::Light),
            ("Checker", PreviewBg::Checker),
            ("Gray", PreviewBg::Gray(128)),
            ("Game", PreviewBg::Game),
        ];
        for (label, candidate) in choices {
            let selected = std::mem::discriminant(&bg) == std::mem::discriminant(&candidate);
            if ui.selectable_label(selected, label).clicked() && !selected {
                bg = candidate;
            }
        }
        if let PreviewBg::Gray(v) = &mut bg {
            ui.add(egui::Slider::new(v, 0..=255).text("Level"));
        }
    });
    // Dark and Light are persisted through the existing settings flag.
    let dark = match bg {
        PreviewBg::Dark => Some(true),
        PreviewBg::Light => Some(false),
        _ => None,
    };
    if let Some(dark) = dark.filter(|d| *d != state.settings.preview_dark_background) {
        state.settings.preview_dark_background = dark;
        state.dirty = true;
    }
    ui_state.bg = Some(bg);

    let mut zoom = state.settings.preview_zoom.clamp(1.0, 16.0);
    let mut guides = ui_state.guides.unwrap_or(true);
    ui.horizontal_wrapped(|ui| {
        ui.label(muted("Zoom"));
        ui.add(egui::Slider::new(&mut zoom, 1.0..=16.0).step_by(1.0).suffix("x"));
        for (label, z) in [("1x", 1.0), ("2x", 2.0), ("4x", 4.0), ("8x", 8.0), ("16x", 16.0)] {
            if ui.small_button(label).clicked() {
                zoom = z;
            }
        }
        if secondary_button(ui, "Center").clicked() {
            ui_state.pan = Vec2::ZERO;
        }
        ui.checkbox(&mut guides, "Center guides");
    });
    ui_state.guides = Some(guides);
    if (zoom - state.settings.preview_zoom).abs() > f32::EPSILON {
        state.settings.preview_zoom = zoom;
        state.dirty = true;
    }

    ui.add_space(8.0);
    let avail = ui.available_size();
    let size = Vec2::new(avail.x.max(200.0), avail.y.max(160.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
    if response.dragged() {
        ui_state.pan += response.drag_delta();
    }

    let painter = ui.painter_at(rect);
    paint_background(&painter, rect, bg);
    let anchor = rect.center() + ui_state.pan;
    if guides {
        let line = Color32::from_rgba_unmultiplied(0x60, 0xA5, 0xFA, 110);
        painter.line_segment(
            [Pos2::new(rect.left(), anchor.y), Pos2::new(rect.right(), anchor.y)],
            Stroke::new(1.0, line),
        );
        painter.line_segment(
            [Pos2::new(anchor.x, rect.top()), Pos2::new(anchor.x, rect.bottom())],
            Stroke::new(1.0, line),
        );
    }
    let bounds = state.editing.bounding_size() as f32;
    paint_crosshair(
        ui,
        Rect::from_center_size(anchor, Vec2::splat(bounds * zoom)),
        &state.editing,
        zoom,
    );
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.bottom() - 12.0),
        Align2::LEFT_BOTTOM,
        format!("Bounding box {} px at {zoom:.0}x. Drag to pan.", bounds as u32),
        FontId::proportional(12.0),
        TEXT2,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
}

fn paint_background(painter: &egui::Painter, rect: Rect, bg: PreviewBg) {
    let radius = CornerRadius::same(10);
    match bg {
        PreviewBg::Dark => {
            painter.rect_filled(rect, radius, theme::PREVIEW_DARK);
        }
        PreviewBg::Light => {
            painter.rect_filled(rect, radius, theme::PREVIEW_LIGHT);
        }
        PreviewBg::Gray(v) => {
            painter.rect_filled(rect, radius, Color32::from_gray(v));
        }
        PreviewBg::Game => {
            painter.rect_filled(rect, radius, Color32::from_rgb(0x4E, 0x5F, 0x4A));
        }
        PreviewBg::Checker => {
            let base = Color32::from_rgb(0x2E, 0x34, 0x40);
            let alt = Color32::from_rgb(0x3B, 0x42, 0x52);
            painter.rect_filled(rect, radius, base);
            let cell = 16.0;
            let cols = (rect.width() / cell).ceil() as i32;
            let rows = (rect.height() / cell).ceil() as i32;
            for row in 0..rows {
                for col in 0..cols {
                    if (row + col) % 2 == 1 {
                        let min = rect.min + Vec2::new(col as f32 * cell, row as f32 * cell);
                        painter.rect_filled(Rect::from_min_size(min, Vec2::splat(cell)), 0.0, alt);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- dialogs

fn dialogs(ctx: &egui::Context, state: &mut AppState, ui_state: &mut EditorUi) {
    reset_dialog(ctx, state, ui_state);
    save_dialog(ctx, state, ui_state);

    if let Some((purpose, mut prompt)) = ui_state.prompt.take() {
        match presets::path_prompt_window(ctx, &mut prompt) {
            PromptOutcome::Open => ui_state.prompt = Some((purpose, prompt)),
            PromptOutcome::Cancel => {}
            PromptOutcome::Confirm(path) => match purpose {
                Purpose::Export => match std::fs::write(&path, &prompt.content) {
                    Ok(()) => state.toast(format!("Exported crosshair to {path}."), false),
                    Err(e) => state.toast(format!("Export failed: {e}"), true),
                },
                Purpose::Import => import_crosshair(state, &path),
            },
        }
    }
}

fn import_crosshair(state: &mut AppState, path: &str) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            state.toast(format!("Could not read {path}: {e}"), true);
            return;
        }
    };
    match parse_import(&text) {
        Ok(data) => {
            let count = data.presets.len();
            match data.presets.into_iter().next() {
                Some(first) => {
                    state.editing = first.crosshair;
                    let extra = if count > 1 {
                        " Only the first crosshair in the file was loaded."
                    } else {
                        ""
                    };
                    state.toast(format!("Loaded \"{}\" into the editor.{extra}", first.name), false);
                }
                None => state.toast("The file contains no crosshair.", true),
            }
        }
        Err(e) => state.toast(format!("Import failed: {e}."), true),
    }
}

fn reset_dialog(ctx: &egui::Context, state: &mut AppState, ui_state: &mut EditorUi) {
    if !ui_state.confirm_reset {
        return;
    }
    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Window::new("Reset crosshair?")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label("Reset the editor to the default crosshair? Unapplied edits are lost. The active overlay is not changed.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if danger_button(ui, "Reset").clicked() {
                    confirmed = true;
                }
                if secondary_button(ui, "Cancel").clicked() {
                    cancelled = true;
                }
            });
        });
    if confirmed {
        state.editing = Crosshair::default();
        state.toast("Editor reset to the default crosshair.", false);
    }
    if confirmed || cancelled || !open {
        ui_state.confirm_reset = false;
    }
}

fn save_dialog(ctx: &egui::Context, state: &mut AppState, ui_state: &mut EditorUi) {
    let Some(mut form) = ui_state.save_form.take() else {
        return;
    };
    let categories = presets::categories(&state.presets);
    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    egui::Window::new("Save as preset")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(muted("Name"));
            ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text("e.g. Tight cross")
                    .desired_width(340.0),
            );
            ui.add_space(6.0);
            ui.label(muted("Category"));
            ui.add(egui::TextEdit::singleline(&mut form.category).desired_width(340.0));
            ui.horizontal_wrapped(|ui| {
                for cat in &categories {
                    if ui.small_button(cat).clicked() {
                        form.category = cat.clone();
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(muted("Description (optional)"));
            ui.add(
                egui::TextEdit::multiline(&mut form.description)
                    .desired_rows(3)
                    .desired_width(340.0),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let valid = !form.name.trim().is_empty();
                if ui.add_enabled(valid, egui::Button::new("Save preset")).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if save {
        let name = form.name.trim().to_string();
        let category = match form.category.trim() {
            "" => "Custom".to_string(),
            c => c.to_string(),
        };
        let description = form.description.trim();
        let preset = Preset {
            name: name.clone(),
            category,
            description: (!description.is_empty()).then(|| description.to_string()),
            crosshair: state.editing.clone(),
            ..Preset::default()
        };
        state.presets.push(preset);
        state.dirty = true;
        state.toast(format!("Saved preset \"{name}\"."), false);
        return;
    }
    if cancel || !open {
        return;
    }
    ui_state.save_form = Some(form);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_preset_found_only_for_identical_crosshair() {
        let custom = Crosshair {
            gap: 9.0,
            ..Crosshair::default()
        };
        let preset = Preset {
            id: "p1".into(),
            crosshair: custom.clone(),
            ..Preset::default()
        };
        let presets = vec![preset];
        assert_eq!(matching_preset_id(&presets, &custom).as_deref(), Some("p1"));
        assert_eq!(matching_preset_id(&presets, &Crosshair::default()), None);
    }
}
