//! Crosshair editor page. A fixed-width, scrollable property panel sits on the left and the preview
//! fills the rest of the page on the right without scrolling. The action bar runs along the bottom,
//! outside both columns.
use crate::crosshair::model::{Crosshair, Preset, Rgba, Shape, limits};
use crate::crosshair::validation;
use crate::state::AppState;
use crate::storage::import_export::{export_crosshair, parse_import};
use crate::ui::presets::{self, FileKind, PathPrompt, PromptOutcome, button_if};
use crate::ui::theme;
use crate::ui::widgets::{
    self, PreviewScene, SCENE_CHECKER_A, caption, collapsing_section, color_swatch, danger_button, ghost_button,
    paint_crosshair, primary_button, scene_color, secondary_button, section_label, segmented, shape_tile, slider_row,
    toggle_row, toggle_switch,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};

const PANEL_W: f32 = 360.0;
/// Vertical space the bottom action bar takes, including the gap above it.
const BAR_RESERVE: f32 = 54.0;
/// Label column of `slider_row`, reused so the colour rows line up with the sliders.
const LABEL_W: f32 = 132.0;
const ROW_H: f32 = 28.0;
const SHAPE_TILE_W: f32 = 96.0;
const SHAPE_GAP: f32 = 8.0;
const SCENES: [(PreviewScene, &str); 4] = [
    (PreviewScene::Dark, "Dark"),
    (PreviewScene::Grey, "Grey"),
    (PreviewScene::Light, "Light"),
    (PreviewScene::Checker, "Checker"),
];

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

/// Per-viewer UI memory (not persisted): scene, pan, hex text, open dialogs and the revert baseline.
#[derive(Clone, Default)]
struct EditorUi {
    scene: Option<PreviewScene>,
    pan: Vec2,
    guides: Option<bool>,
    live: Option<bool>,
    main_hex: HexField,
    outline_hex: HexField,
    confirm_reset: bool,
    save_form: Option<SaveForm>,
    prompt: Option<(Purpose, PathPrompt)>,
    test: Option<TestSession>,
    /// Crosshair that Revert returns to: the last applied one, or whatever another page applied.
    baseline: Option<Crosshair>,
    /// Crosshair this page last wrote to `settings.active_crosshair`. A different value there means
    /// another page changed it.
    written: Option<Crosshair>,
}

/// Preset whose crosshair equals `c`, used to keep `active_preset_id` honest.
fn matching_preset_id(presets: &[Preset], c: &Crosshair) -> Option<String> {
    presets.iter().find(|p| p.crosshair == *c).map(|p| p.id.clone())
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("editor_page_state");
    let mut ui_state: EditorUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();
    if ui_state.written.as_ref() != Some(&state.settings.active_crosshair) {
        // Changed outside this page (for example a preset applied on the Presets page).
        ui_state.baseline = Some(state.settings.active_crosshair.clone());
        ui_state.written = Some(state.settings.active_crosshair.clone());
    }
    let before = state.editing.clone();

    let body_h = (ui.available_height() - BAR_RESERVE).max(240.0);
    ui.vertical(|ui| {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(PANEL_W, body_h), Layout::top_down(Align::Min), |ui| {
                properties_panel(ui, state, &mut ui_state);
            });
            let right_w = ui.available_width().max(200.0);
            ui.allocate_ui_with_layout(Vec2::new(right_w, body_h), Layout::top_down(Align::Min), |ui| {
                preview_panel(ui, state, &mut ui_state);
            });
        });
        action_bar(ui, state, &mut ui_state);
    });

    dialogs(ui.ctx(), state, &mut ui_state);

    // Clamp after edits, then push changes to the overlay when the editor drives it.
    validation::clamp(&mut state.editing);
    if state.editing != before {
        let live_on = ui_state.live.unwrap_or(state.settings.overlay_enabled);
        let driving_overlay = ui_state.test.is_some() || (live_on && state.settings.overlay_enabled);
        if driving_overlay {
            let c = state.editing.clone();
            write_active(state, &mut ui_state, c);
            state.settings.active_preset_id = matching_preset_id(&state.presets, &state.editing);
            state.push_overlay();
            state.dirty = true;
        }
    }

    ui.data_mut(|d| d.insert_temp(memory_id, ui_state));
}

/// Store `c` as the active crosshair and remember that this page wrote it.
fn write_active(state: &mut AppState, ui_state: &mut EditorUi, c: Crosshair) {
    ui_state.written = Some(c.clone());
    state.settings.active_crosshair = c;
}

// ---------------------------------------------------------------- action bar

/// Full-width bar below both columns: Revert, Reset, Import, Export on the left; Test overlay,
/// Live apply, Save as preset and Apply on the right.
fn action_bar(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    let p = theme::palette(ui.ctx());
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if secondary_button(ui, "Revert").clicked() {
                    let target = ui_state.baseline.clone().unwrap_or_else(|| state.editing.clone());
                    state.editing = target;
                    state.toast("Reverted to the applied crosshair.", false);
                }
                if secondary_button(ui, "Reset").clicked() {
                    ui_state.confirm_reset = true;
                }
                if secondary_button(ui, "Import").clicked() {
                    ui_state.prompt = Some((
                        Purpose::Import,
                        PathPrompt::new("Import crosshair", false, FileKind::Json, "", String::new()),
                    ));
                }
                if secondary_button(ui, "Export").clicked() {
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
                // Right-to-left: the first widget added sits furthest right.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, "Apply").clicked() {
                        apply(state, ui_state);
                    }
                    if secondary_button(ui, "Save as preset").clicked() {
                        ui_state.save_form = Some(SaveForm {
                            category: "Custom".into(),
                            ..SaveForm::default()
                        });
                    }
                    let mut live = ui_state.live.unwrap_or(state.settings.overlay_enabled);
                    ui.horizontal(|ui| {
                        if toggle_switch(ui, &mut live).changed() {
                            ui_state.live = Some(live);
                        }
                        ui.label(RichText::new("Live apply").size(14.0).color(p.text));
                    });
                    let mut testing = ui_state.test.is_some();
                    ui.horizontal(|ui| {
                        if toggle_switch(ui, &mut testing).changed() {
                            if testing {
                                start_test(state, ui_state);
                            } else {
                                stop_test(state, ui_state);
                            }
                        }
                        ui.label(RichText::new("Test overlay").size(14.0).color(p.text));
                    });
                });
            });
        });
}

fn apply(state: &mut AppState, ui_state: &mut EditorUi) {
    if let Some(test) = ui_state.test.take() {
        // Applying ends the temporary test: keep the new crosshair, restore the overlay flags.
        state.settings.overlay_enabled = test.prev_enabled;
        state.settings.crosshair_visible = test.prev_visible;
    }
    let c = state.editing.clone();
    write_active(state, ui_state, c.clone());
    ui_state.baseline = Some(c);
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
    let c = state.editing.clone();
    write_active(state, ui_state, c);
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
        write_active(state, ui_state, test.prev_active);
        state.dirty = true;
        state.push_overlay();
        state.toast("Test overlay stopped. The previous overlay state is restored.", false);
    }
}

// ---------------------------------------------------------------- property panel

fn properties_panel(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    let c = &mut state.editing;
    widgets::card(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            section_label(ui, "Shape");
            shape_grid(ui, &mut c.shape);
            ui.add_space(6.0);

            collapsing_section(ui, "geometry", "Geometry", true, |ui| {
                slider_row(
                    ui,
                    "Horizontal length",
                    &mut c.length_h,
                    0.0..=limits::MAX_LENGTH,
                    "px",
                    1.0,
                );
                slider_row(
                    ui,
                    "Vertical length",
                    &mut c.length_v,
                    0.0..=limits::MAX_LENGTH,
                    "px",
                    1.0,
                );
                slider_row(
                    ui,
                    "Thickness",
                    &mut c.thickness,
                    0.0..=limits::MAX_THICKNESS,
                    "px",
                    0.5,
                );
                slider_row(ui, "Gap", &mut c.gap, 0.0..=limits::MAX_GAP, "px", 1.0);
                slider_row(
                    ui,
                    "Circle radius",
                    &mut c.circle_radius,
                    0.0..=limits::MAX_RADIUS,
                    "px",
                    1.0,
                );
                slider_row(ui, "Dot size", &mut c.dot_size, 0.0..=limits::MAX_DOT, "px", 0.5);
                slider_row(ui, "Rotation", &mut c.rotation, 0.0..=360.0, "\u{b0}", 1.0);
            });

            collapsing_section(ui, "colour", "Colour", true, |ui| {
                color_editor(ui, &mut c.color, &mut ui_state.main_hex);
                ui.add_space(4.0);
                let mut percent = c.opacity * 100.0;
                if slider_row(ui, "Opacity", &mut percent, 0.0..=100.0, "%", 1.0) {
                    c.opacity = (percent / 100.0).clamp(0.0, 1.0);
                }
            });

            collapsing_section(ui, "outline", "Outline & shadow", false, |ui| {
                toggle_row(ui, "Outline", None, &mut c.outline);
                ui.add_enabled_ui(c.outline, |ui| {
                    slider_row(
                        ui,
                        "Outline thickness",
                        &mut c.outline_thickness,
                        0.0..=limits::MAX_OUTLINE,
                        "px",
                        0.5,
                    );
                    color_editor(ui, &mut c.outline_color, &mut ui_state.outline_hex);
                });
                ui.add_space(6.0);
                toggle_row(ui, "Shadow", None, &mut c.shadow);
                ui.add_enabled_ui(c.shadow, |ui| {
                    slider_row(
                        ui,
                        "Shadow offset",
                        &mut c.shadow_offset,
                        0.0..=validation::MAX_SHADOW_OFFSET,
                        "px",
                        0.5,
                    );
                    let mut percent = c.shadow_opacity * 100.0;
                    if slider_row(ui, "Shadow opacity", &mut percent, 0.0..=100.0, "%", 1.0) {
                        c.shadow_opacity = (percent / 100.0).clamp(0.0, 1.0);
                    }
                });
            });

            collapsing_section(ui, "elements", "Elements", false, |ui| {
                toggle_row(ui, "Lines", None, &mut c.show_lines);
                toggle_row(ui, "Center dot", None, &mut c.show_dot);
                toggle_row(ui, "Circle", None, &mut c.show_circle);
            });

            collapsing_section(ui, "offset", "Position offset", false, |ui| {
                slider_row(
                    ui,
                    "Offset X",
                    &mut c.offset_x,
                    -limits::MAX_OFFSET..=limits::MAX_OFFSET,
                    "px",
                    1.0,
                );
                slider_row(
                    ui,
                    "Offset Y",
                    &mut c.offset_y,
                    -limits::MAX_OFFSET..=limits::MAX_OFFSET,
                    "px",
                    1.0,
                );
            });
        });
    });
}

/// Grid of shape tiles, columns computed from the available width.
fn shape_grid(ui: &mut egui::Ui, shape: &mut Shape) {
    let p = theme::palette(ui.ctx());
    let cols = presets::grid_columns(ui.available_width(), SHAPE_TILE_W, SHAPE_GAP);
    for row in Shape::ALL.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SHAPE_GAP;
            for &s in row {
                let selected = *shape == s;
                let color = if selected { p.primary } else { p.text };
                if shape_tile(ui, selected, s.label(), |painter, rect| {
                    paint_shape(painter, rect, s, color);
                })
                .clicked()
                {
                    *shape = s;
                }
            }
        });
    }
}

/// Small schematic of a shape, drawn inside the tile's preview area.
fn paint_shape(painter: &egui::Painter, rect: Rect, shape: Shape, color: Color32) {
    let c = rect.center();
    let arm = rect.height().min(rect.width()) * 0.5 - 6.0;
    let gap = 5.0;
    let stroke = Stroke::new(2.0, color);
    let cross = |painter: &egui::Painter| {
        for dir in [
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, -1.0),
        ] {
            painter.line_segment([c + dir * gap, c + dir * (gap + arm)], stroke);
        }
    };
    let dot = |painter: &egui::Painter| painter.circle_filled(c, 2.5, color);
    match shape {
        Shape::Cross => cross(painter),
        Shape::Dot => {
            painter.circle_filled(c, 3.5, color);
        }
        Shape::Circle => {
            painter.circle_stroke(c, arm * 0.8, stroke);
        }
        Shape::CrossDot => {
            cross(painter);
            dot(painter);
        }
        Shape::CrossNoOuter => {
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let pos = c + Vec2::new(a.cos(), a.sin()) * (arm * 0.6);
                painter.circle_filled(pos, 1.8, color);
            }
        }
        Shape::TShape => {
            painter.line_segment(
                [Pos2::new(c.x - arm, c.y - gap), Pos2::new(c.x + arm, c.y - gap)],
                stroke,
            );
            painter.line_segment([Pos2::new(c.x, c.y - gap), Pos2::new(c.x, c.y + arm)], stroke);
        }
        Shape::Chevron => {
            painter.add(egui::Shape::line(
                vec![
                    Pos2::new(c.x - arm, c.y + arm * 0.4),
                    Pos2::new(c.x, c.y - arm * 0.4),
                    Pos2::new(c.x + arm, c.y + arm * 0.4),
                ],
                stroke,
            ));
        }
    }
}

/// Label column of a colour row, same width as the `slider_row` labels.
fn row_label(ui: &mut egui::Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.allocate_ui_with_layout(Vec2::new(LABEL_W, ROW_H), Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new(text).size(14.0).color(p.text));
    });
}

/// Swatch and HEX input on one row, then Red / Green / Blue sliders.
fn color_editor(ui: &mut egui::Ui, color: &mut Rgba, hex: &mut HexField) {
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        row_label(ui, "Hex");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut hex.text)
                .hint_text("#RRGGBB")
                .desired_width(120.0),
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
        color_swatch(
            ui,
            Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
            false,
        );
    });
    if hex.invalid {
        ui.label(RichText::new("Use #RRGGBB").size(12.0).color(p.danger));
    }

    let mut r = f32::from(color.r);
    let mut g = f32::from(color.g);
    let mut b = f32::from(color.b);
    if slider_row(ui, "Red", &mut r, 0.0..=255.0, "", 1.0) {
        color.r = r.round() as u8;
    }
    if slider_row(ui, "Green", &mut g, 0.0..=255.0, "", 1.0) {
        color.g = g.round() as u8;
    }
    if slider_row(ui, "Blue", &mut b, 0.0..=255.0, "", 1.0) {
        color.b = b.round() as u8;
    }

    // Refresh the text when the colour changed from the sliders (or elsewhere).
    if hex.last != Some(*color) {
        hex.text = color.to_hex();
        hex.last = Some(*color);
        hex.invalid = false;
    }
}

// ---------------------------------------------------------------- preview

fn preview_panel(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut EditorUi) {
    let p = theme::palette(ui.ctx());
    let fallback = if state.settings.preview_dark_background {
        PreviewScene::Dark
    } else {
        PreviewScene::Light
    };
    let mut scene = ui_state.scene.unwrap_or(fallback);
    let mut guides = ui_state.guides.unwrap_or(true);
    let mut recenter = false;
    ui.horizontal(|ui| {
        segmented(ui, &mut scene, &SCENES);
        if secondary_button(ui, "Center").clicked() {
            recenter = true;
        }
        ui.add_space(8.0);
        toggle_switch(ui, &mut guides);
        ui.label(RichText::new("Guides").size(14.0).color(p.text));
    });
    if recenter {
        ui_state.pan = Vec2::ZERO;
    }
    ui_state.scene = Some(scene);
    ui_state.guides = Some(guides);
    // Dark and Light are the two scenes kept in settings; Grey and Checker last for the session.
    let stored = match scene {
        PreviewScene::Dark => Some(true),
        PreviewScene::Light => Some(false),
        _ => None,
    };
    if let Some(dark) = stored.filter(|d| *d != state.settings.preview_dark_background) {
        state.settings.preview_dark_background = dark;
        state.dirty = true;
    }

    let mut zoom = state.settings.preview_zoom.clamp(1.0, 16.0);
    slider_row(ui, "Zoom", &mut zoom, 1.0..=16.0, "x", 1.0);
    if (zoom - state.settings.preview_zoom).abs() > f32::EPSILON {
        state.settings.preview_zoom = zoom;
        state.dirty = true;
    }

    ui.add_space(4.0);
    let avail = ui.available_size();
    let size = Vec2::new(avail.x.max(200.0), avail.y.max(160.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
    if response.dragged() {
        ui_state.pan += response.drag_delta();
    }
    let half = rect.size() * 0.5;
    ui_state.pan = Vec2::new(
        ui_state.pan.x.clamp(-half.x, half.x),
        ui_state.pan.y.clamp(-half.y, half.y),
    );

    let painter = ui.painter_at(rect);
    paint_scene(&painter, rect, scene);
    let anchor = rect.center() + ui_state.pan;
    if guides {
        let guide = p.accent.gamma_multiply(0.6);
        painter.line_segment(
            [Pos2::new(rect.left(), anchor.y), Pos2::new(rect.right(), anchor.y)],
            Stroke::new(1.0, guide),
        );
        painter.line_segment(
            [Pos2::new(anchor.x, rect.top()), Pos2::new(anchor.x, rect.bottom())],
            Stroke::new(1.0, guide),
        );
    }
    let bounds = state.editing.bounding_size();
    let side = bounds as f32 * zoom;
    // The crosshair may be larger than the preview at high zoom: clip it to the preview area.
    let outer_clip = ui.clip_rect();
    ui.set_clip_rect(rect);
    paint_crosshair(
        ui,
        Rect::from_center_size(anchor, Vec2::splat(side)),
        &state.editing,
        zoom,
    );
    ui.set_clip_rect(outer_clip);
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.bottom() - 12.0),
        Align2::LEFT_BOTTOM,
        format!("Bounding box {bounds} px at {zoom:.0}x. Drag to pan."),
        FontId::proportional(12.0),
        p.text_muted,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
}

/// Scene background for the preview. Fixed colors on purpose, independent of the app theme.
fn paint_scene(painter: &egui::Painter, rect: Rect, scene: PreviewScene) {
    painter.rect_filled(rect, 6.0, scene_color(scene));
    if scene == PreviewScene::Checker {
        const CELL: f32 = 12.0;
        let cols = (rect.width() / CELL).ceil() as usize;
        let rows = (rect.height() / CELL).ceil() as usize;
        for row in 0..rows {
            for col in 0..cols {
                if (row + col) % 2 == 0 {
                    let min = rect.min + Vec2::new(col as f32 * CELL, row as f32 * CELL);
                    painter.rect_filled(Rect::from_min_size(min, Vec2::splat(CELL)), 0.0, SCENE_CHECKER_A);
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
            ui.label(
                "Reset the editor to the default crosshair? Unapplied edits are lost. The active overlay is not changed.",
            );
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
            caption(ui, "Name");
            ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text("e.g. Tight cross")
                    .desired_width(340.0),
            );
            ui.add_space(6.0);
            caption(ui, "Category");
            ui.add(egui::TextEdit::singleline(&mut form.category).desired_width(340.0));
            ui.horizontal_wrapped(|ui| {
                for cat in &categories {
                    if ghost_button(ui, cat).clicked() {
                        form.category = cat.clone();
                    }
                }
            });
            ui.add_space(6.0);
            caption(ui, "Description (optional)");
            ui.add(
                egui::TextEdit::multiline(&mut form.description)
                    .desired_rows(3)
                    .desired_width(340.0),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let valid = !form.name.trim().is_empty();
                if button_if(ui, valid, primary_button, "Save preset") {
                    save = true;
                }
                if secondary_button(ui, "Cancel").clicked() {
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

    #[test]
    fn shape_grid_fits_the_property_panel_in_three_columns() {
        // Panel inner width is about 318 px: three 96 px tiles with 8 px gaps fit, four do not.
        assert_eq!(presets::grid_columns(318.0, SHAPE_TILE_W, SHAPE_GAP), 3);
        assert_eq!(presets::grid_columns(80.0, SHAPE_TILE_W, SHAPE_GAP), 1);
    }

    #[test]
    fn scene_chips_cover_every_preview_scene() {
        for scene in [
            PreviewScene::Dark,
            PreviewScene::Grey,
            PreviewScene::Light,
            PreviewScene::Checker,
        ] {
            assert!(SCENES.iter().any(|(s, _)| *s == scene));
        }
    }
}
