//! Preset library page. Also hosts helpers shared with the editor and profiles pages:
//! the file path prompt (text field + Browse on Windows) and preset categories.
use crate::crosshair::model::Preset;
use crate::state::{AppState, Page};
use crate::storage::import_export::{export_collection, export_crosshair, merge_imports, parse_import};
use crate::ui::theme::{self, BLUE, BORDER, SURFACE, TEXT, TEXT2};
use crate::ui::widgets::{danger_button, paint_crosshair, secondary_button};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::path::{Path, PathBuf};

const CARD_W: f32 = 220.0;
const CARD_H: f32 = 176.0;
const TILE_H: f32 = 112.0;

/// Distinct, sorted (case-insensitive) categories used by the given presets.
pub(crate) fn categories(presets: &[Preset]) -> Vec<String> {
    let mut out: Vec<String> = presets
        .iter()
        .map(|p| p.category.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    out.sort_by_key(|c| c.to_lowercase());
    out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    out
}

/// Turn a preset name into a safe default file name.
pub(crate) fn file_name_for(name: &str) -> String {
    let mut s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-');
    if s.is_empty() {
        "crosshair.json".to_string()
    } else {
        format!("{s}.json")
    }
}

/// Kind of file a path prompt works with (controls the Browse filter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FileKind {
    #[default]
    Json,
    Exe,
}

/// Whether the Browse button can open a native dialog on this platform.
pub(crate) const CAN_BROWSE: bool = cfg!(windows);

/// Native open/save dialog. Returns None on platforms without a native dialog or when cancelled.
pub(crate) fn pick_path(kind: FileKind, save: bool, suggested: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let (label, ext) = match kind {
            FileKind::Json => ("JSON files", "json"),
            FileKind::Exe => ("Applications", "exe"),
        };
        let dialog = rfd::FileDialog::new().add_filter(label, &[ext]);
        if save {
            dialog.set_file_name(suggested).save_file()
        } else {
            dialog.pick_file()
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (kind, save, suggested);
        None
    }
}

/// A modal asking for a file path. Used for import and export on every platform.
#[derive(Clone, Debug, Default)]
pub(crate) struct PathPrompt {
    pub title: String,
    pub save: bool,
    pub kind: FileKind,
    pub path: String,
    /// Text written on confirm when `save` is true.
    pub content: String,
}

impl PathPrompt {
    pub(crate) fn new(title: &str, save: bool, kind: FileKind, suggested: &str, content: String) -> Self {
        Self {
            title: title.to_string(),
            save,
            kind,
            path: suggested.to_string(),
            content,
        }
    }
}

pub(crate) enum PromptOutcome {
    Open,
    Cancel,
    Confirm(String),
}

/// Renders the path prompt as a centered window. Returns what the user did this frame.
pub(crate) fn path_prompt_window(ctx: &egui::Context, prompt: &mut PathPrompt) -> PromptOutcome {
    let mut outcome = PromptOutcome::Open;
    let mut open = true;
    let title = prompt.title.clone();
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(440.0);
            let hint = match (prompt.save, CAN_BROWSE) {
                (true, true) => "Choose where to save the file, or type a full path.",
                (false, true) => "Choose a file, or type its full path.",
                (true, false) => "Type the full path of the file to write.",
                (false, false) => "Type the full path of the file to open.",
            };
            ui.label(egui::RichText::new(hint).size(12.0).color(TEXT2));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let width = if CAN_BROWSE { 300.0 } else { 420.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.path)
                        .hint_text("C:\\path\\to\\file.json")
                        .desired_width(width),
                );
                if CAN_BROWSE && secondary_button(ui, "Browse...").clicked() {
                    if let Some(p) = pick_path(prompt.kind, prompt.save, &prompt.path) {
                        prompt.path = p.display().to_string();
                    }
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let ok = !prompt.path.trim().is_empty();
                let label = if prompt.save { "Save" } else { "Open" };
                if ui.add_enabled(ok, egui::Button::new(label)).clicked() {
                    outcome = PromptOutcome::Confirm(prompt.path.trim().to_string());
                }
                if ui.button("Cancel").clicked() {
                    outcome = PromptOutcome::Cancel;
                }
            });
        });
    if !open && matches!(outcome, PromptOutcome::Open) {
        outcome = PromptOutcome::Cancel;
    }
    outcome
}

/// Muted helper text style used across the editor, presets and profiles pages.
pub(crate) fn muted(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(12.0).color(TEXT2)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Transfer {
    Import,
    Export,
}

#[derive(Clone, Default)]
struct MetaForm {
    id: String,
    name: String,
    category: String,
    description: String,
}

#[derive(Clone, Default)]
struct PresetsUi {
    query: String,
    category: Option<String>,
    favorites_only: bool,
    meta: Option<MetaForm>,
    delete: Option<String>,
    prompt: Option<(Transfer, PathPrompt)>,
}

enum Action {
    Apply(String),
    Open(String),
    Edit(String),
    Duplicate(String),
    Export(String),
    Delete(String),
    Favorite(String),
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("presets_page_state");
    let mut pu: PresetsUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();

    toolbar(ui, state, &mut pu);
    ui.add_space(8.0);
    category_chips(ui, state, &mut pu);
    ui.add_space(12.0);

    let mut actions = Vec::new();
    grid(ui, state, &pu, &mut actions);
    for action in actions {
        handle_action(state, &mut pu, action);
    }

    dialogs(ui.ctx(), state, &mut pu);
    ui.data_mut(|d| d.insert_temp(memory_id, pu));
}

fn toolbar(ui: &mut egui::Ui, state: &mut AppState, pu: &mut PresetsUi) {
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut pu.query)
                .hint_text("Search by name")
                .desired_width(220.0),
        );
        ui.toggle_value(&mut pu.favorites_only, "Favorites only");
        ui.add_space(8.0);
        if secondary_button(ui, "New from current").clicked() {
            let mut preset = Preset {
                name: format!("Crosshair {}", state.presets.len() + 1),
                category: "Custom".into(),
                ..Preset::default()
            };
            preset.crosshair = state.editing.clone();
            let name = preset.name.clone();
            state.presets.push(preset);
            state.dirty = true;
            state.toast(format!("Saved the editor crosshair as \"{name}\"."), false);
        }
        if secondary_button(ui, "Import").clicked() {
            pu.prompt = Some((
                Transfer::Import,
                PathPrompt::new("Import presets", false, FileKind::Json, "", String::new()),
            ));
        }
        let can_export = !state.presets.is_empty();
        if ui.add_enabled(can_export, egui::Button::new("Export all")).clicked() {
            let content = export_collection(&state.presets);
            pu.prompt = Some((
                Transfer::Export,
                PathPrompt::new(
                    "Export all presets",
                    true,
                    FileKind::Json,
                    "opencrosshair-presets.json",
                    content,
                ),
            ));
        }
    });
}

fn category_chips(ui: &mut egui::Ui, state: &AppState, pu: &mut PresetsUi) {
    let cats = categories(&state.presets);
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(pu.category.is_none(), "All").clicked() {
            pu.category = None;
        }
        for cat in &cats {
            let selected = pu.category.as_deref() == Some(cat.as_str());
            if ui.selectable_label(selected, cat).clicked() {
                pu.category = if selected { None } else { Some(cat.clone()) };
            }
        }
    });
}

fn grid(ui: &mut egui::Ui, state: &AppState, pu: &PresetsUi, actions: &mut Vec<Action>) {
    if state.presets.is_empty() {
        ui.label(muted(
            "No presets yet. Save a crosshair from the editor or import a file.",
        ));
        return;
    }
    let query = pu.query.trim().to_lowercase();
    let visible: Vec<&Preset> = state
        .presets
        .iter()
        .filter(|p| !pu.favorites_only || p.favorite)
        .filter(|p| pu.category.as_ref().is_none_or(|c| &p.category == c))
        .filter(|p| {
            query.is_empty() || p.name.to_lowercase().contains(&query) || p.category.to_lowercase().contains(&query)
        })
        .collect();
    if visible.is_empty() {
        ui.label(muted("No presets match the current filters."));
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(16.0);
            for p in visible {
                card(ui, p, actions);
            }
        });
    });
}

fn card(ui: &mut egui::Ui, p: &Preset, actions: &mut Vec<Action>) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(CARD_W, CARD_H), Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let painter = ui.painter_at(rect);
    let border = if hovered { BLUE } else { BORDER };
    painter.rect_filled(rect, CornerRadius::same(12), SURFACE);
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    let tile = Rect::from_min_size(rect.min + Vec2::splat(8.0), Vec2::new(CARD_W - 16.0, TILE_H - 8.0));
    painter.rect_filled(tile, CornerRadius::same(8), theme::PREVIEW_DARK);
    let bounds = p.crosshair.bounding_size() as f32;
    let zoom = (88.0 / bounds).clamp(0.5, 3.0);
    paint_crosshair(
        ui,
        Rect::from_center_size(tile.center(), Vec2::splat(bounds * zoom)),
        &p.crosshair,
        zoom,
    );

    let text_x = rect.left() + 12.0;
    let base_y = tile.bottom() + 10.0;
    painter.text(
        Pos2::new(text_x, base_y),
        Align2::LEFT_TOP,
        &p.name,
        FontId::proportional(14.0),
        TEXT,
    );
    let meta = if p.builtin {
        format!("{} · Built-in", p.category)
    } else {
        p.category.clone()
    };
    painter.text(
        Pos2::new(text_x, base_y + 22.0),
        Align2::LEFT_TOP,
        meta,
        FontId::proportional(12.0),
        TEXT2,
    );

    // Favorite star, top-right of the tile.
    let star_rect = Rect::from_min_size(Pos2::new(tile.right() - 28.0, tile.top() + 6.0), Vec2::splat(22.0));
    let star = ui.interact(star_rect, ui.scope_id().with(("preset_fav", &p.id)), Sense::click());
    draw_star(&painter, star_rect.center(), 8.5, p.favorite, star.hovered());
    if star.clicked() {
        actions.push(Action::Favorite(p.id.clone()));
    } else if resp.clicked() {
        actions.push(Action::Apply(p.id.clone()));
    }

    if hovered {
        painter.rect_filled(tile, CornerRadius::same(8), Color32::from_black_alpha(175));
        let c = tile.center();
        let row1 = Rect::from_min_size(Pos2::new(tile.left() + 4.0, c.y - 30.0), Vec2::new(96.0, 26.0));
        let row1b = Rect::from_min_size(Pos2::new(tile.left() + 104.0, c.y - 30.0), Vec2::new(96.0, 26.0));
        let row2_y = c.y + 4.0;
        let row2 = [
            Rect::from_min_size(Pos2::new(tile.left() + 4.0, row2_y), Vec2::new(62.0, 26.0)),
            Rect::from_min_size(Pos2::new(tile.left() + 70.0, row2_y), Vec2::new(62.0, 26.0)),
            Rect::from_min_size(Pos2::new(tile.left() + 136.0, row2_y), Vec2::new(62.0, 26.0)),
        ];
        let id = p.id.clone();
        if action_button(ui, row1, "Apply", true, true) {
            actions.push(Action::Apply(id.clone()));
        }
        let second = if p.builtin { "Open" } else { "Edit" };
        if action_button(ui, row1b, second, true, false) {
            actions.push(if p.builtin {
                Action::Open(id.clone())
            } else {
                Action::Edit(id.clone())
            });
        }
        if action_button(ui, row2[0], "Duplicate", true, false) {
            actions.push(Action::Duplicate(id.clone()));
        }
        if action_button(ui, row2[1], "Export", true, false) {
            actions.push(Action::Export(id.clone()));
        }
        if action_button(ui, row2[2], "Delete", !p.builtin, false) {
            actions.push(Action::Delete(id));
        }
    }
}

fn action_button(ui: &mut egui::Ui, rect: Rect, label: &str, enabled: bool, primary: bool) -> bool {
    let text = egui::RichText::new(label).size(12.0);
    let text = if primary {
        text.strong().color(Color32::WHITE)
    } else {
        text.color(TEXT)
    };
    let fill = if primary { BLUE } else { SURFACE };
    ui.add_enabled_ui(enabled, |ui| {
        ui.put(
            rect,
            egui::Button::new(text)
                .fill(fill)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(CornerRadius::same(6))
                .min_size(rect.size()),
        )
        .clicked()
    })
    .inner
}

fn draw_star(painter: &egui::Painter, center: Pos2, radius: f32, filled: bool, hovered: bool) {
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let r = if i % 2 == 0 { radius } else { radius * 0.42 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(center + Vec2::new(a.cos(), a.sin()) * r);
    }
    let color = if filled {
        BLUE
    } else if hovered {
        TEXT
    } else {
        TEXT2
    };
    if filled {
        // Fan of convex triangles from the center; a concave star cannot be filled directly.
        for i in 0..10 {
            let a = points[i];
            let b = points[(i + 1) % 10];
            painter.add(egui::Shape::convex_polygon(vec![center, a, b], color, Stroke::NONE));
        }
    } else {
        let mut closed = points.clone();
        closed.push(points[0]);
        painter.add(egui::Shape::line(closed, Stroke::new(1.4, color)));
    }
}

fn open_in_editor(state: &mut AppState, preset: &Preset) {
    state.editing = preset.crosshair.clone();
    state.page = Page::Editor;
}

fn find<'a>(state: &'a AppState, id: &str) -> Option<&'a Preset> {
    state.presets.iter().find(|p| p.id == id)
}

fn handle_action(state: &mut AppState, pu: &mut PresetsUi, action: Action) {
    match action {
        Action::Apply(id) => {
            if let Some(name) = find(state, &id).map(|p| p.name.clone()) {
                state.apply_preset(&id);
                state.toast(format!("Active crosshair is now \"{name}\"."), false);
            }
        }
        Action::Open(id) => {
            if let Some(p) = find(state, &id).cloned() {
                open_in_editor(state, &p);
            }
        }
        Action::Edit(id) => {
            if let Some(p) = find(state, &id) {
                pu.meta = Some(MetaForm {
                    id,
                    name: p.name.clone(),
                    category: p.category.clone(),
                    description: p.description.clone().unwrap_or_default(),
                });
            }
        }
        Action::Duplicate(id) => {
            let Some(original) = find(state, &id).cloned() else {
                return;
            };
            let fresh = Preset::default();
            let mut copy = original.clone();
            copy.id = fresh.id;
            copy.created_at = fresh.created_at;
            copy.name = format!("{} (copy)", original.name);
            copy.builtin = false;
            copy.favorite = false;
            let pos = state
                .presets
                .iter()
                .position(|p| p.id == id)
                .map_or(state.presets.len(), |i| i + 1);
            let name = copy.name.clone();
            state.presets.insert(pos, copy);
            state.dirty = true;
            state.toast(
                format!("Created \"{name}\". Built-in presets are copied, not edited."),
                false,
            );
        }
        Action::Export(id) => {
            if let Some(p) = find(state, &id) {
                let prompt = PathPrompt::new(
                    "Export preset",
                    true,
                    FileKind::Json,
                    &file_name_for(&p.name),
                    export_crosshair(p),
                );
                pu.prompt = Some((Transfer::Export, prompt));
            }
        }
        Action::Delete(id) => {
            if find(state, &id).is_some_and(|p| p.builtin) {
                state.toast("Built-in presets cannot be deleted. Duplicate it instead.", true);
            } else {
                pu.delete = Some(id);
            }
        }
        Action::Favorite(id) => {
            if let Some(p) = state.presets.iter_mut().find(|p| p.id == id) {
                p.favorite = !p.favorite;
                state.dirty = true;
            }
        }
    }
}

fn dialogs(ctx: &egui::Context, state: &mut AppState, pu: &mut PresetsUi) {
    delete_dialog(ctx, state, pu);
    meta_dialog(ctx, state, pu);

    if let Some((transfer, mut prompt)) = pu.prompt.take() {
        match path_prompt_window(ctx, &mut prompt) {
            PromptOutcome::Open => pu.prompt = Some((transfer, prompt)),
            PromptOutcome::Cancel => {}
            PromptOutcome::Confirm(path) => run_transfer(state, transfer, &prompt, Path::new(&path)),
        }
    }
}

fn run_transfer(state: &mut AppState, transfer: Transfer, prompt: &PathPrompt, path: &Path) {
    match transfer {
        Transfer::Export => match std::fs::write(path, &prompt.content) {
            Ok(()) => state.toast(format!("Exported to {}.", path.display()), false),
            Err(e) => state.toast(format!("Export failed: {e}"), true),
        },
        Transfer::Import => {
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    state.toast(format!("Could not read {}: {e}", path.display()), true);
                    return;
                }
            };
            match import_text(state, &text) {
                Ok(msg) => state.toast(msg, false),
                Err(msg) => state.toast(msg, true),
            }
        }
    }
}

/// Parse an export document and merge its presets into the library.
pub(crate) fn import_text(state: &mut AppState, text: &str) -> Result<String, String> {
    let data = parse_import(text).map_err(|e| format!("Import failed: {e}."))?;
    let count = data.presets.len();
    let report = merge_imports(&mut state.presets, data.presets);
    state.dirty = true;
    let mut msg = format!("Imported {} of {} preset(s).", report.added, count);
    if report.reassigned_ids + report.renamed.len() > 0 {
        msg.push_str(&format!(" {} renamed to avoid duplicates.", report.renamed.len()));
    }
    for w in data.warnings.iter().take(1) {
        msg.push_str(&format!(" {w}"));
    }
    Ok(msg)
}

fn delete_dialog(ctx: &egui::Context, state: &mut AppState, pu: &mut PresetsUi) {
    let Some(id) = pu.delete.clone() else { return };
    let name = find(state, &id).map(|p| p.name.clone()).unwrap_or_default();
    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Window::new("Delete preset?")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.label(format!(
                "\"{name}\" will be removed from your library. This cannot be undone."
            ));
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
        let before = state.presets.len();
        state.presets.retain(|p| !(p.id == id && !p.builtin));
        if state.presets.len() != before {
            if state.settings.active_preset_id.as_deref() == Some(id.as_str()) {
                state.settings.active_preset_id = None;
            }
            state.dirty = true;
            state.toast(format!("Deleted \"{name}\"."), false);
        }
    }
    if confirmed || cancelled || !open {
        pu.delete = None;
    }
}

fn meta_dialog(ctx: &egui::Context, state: &mut AppState, pu: &mut PresetsUi) {
    let Some(mut form) = pu.meta.take() else { return };
    let categories = categories(&state.presets);
    let mut open = true;
    let mut save = false;
    let mut open_editor = false;
    let mut cancel = false;
    egui::Window::new("Edit preset")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(muted("Name"));
            ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(340.0));
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
                if ui.add_enabled(valid, egui::Button::new("Save")).clicked() {
                    save = true;
                }
                if secondary_button(ui, "Open in editor").clicked() {
                    open_editor = true;
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
        if let Some(p) = state.presets.iter_mut().find(|p| p.id == form.id && !p.builtin) {
            p.name = name.clone();
            p.category = category;
            p.description = (!description.is_empty()).then(|| description.to_string());
            state.dirty = true;
            state.toast(format!("Saved \"{name}\"."), false);
        } else {
            state.toast("That preset no longer exists.", true);
        }
        return;
    }
    if open_editor {
        if let Some(p) = find(state, &form.id).cloned() {
            open_in_editor(state, &p);
        }
        return;
    }
    if cancel || !open {
        return;
    }
    pu.meta = Some(form);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str, category: &str) -> Preset {
        Preset {
            name: name.into(),
            category: category.into(),
            ..Preset::default()
        }
    }

    #[test]
    fn categories_are_trimmed_unique_and_sorted() {
        let presets = vec![
            preset("a", "Pro"),
            preset("b", "custom"),
            preset("c", " Custom "),
            preset("d", "  "),
            preset("e", "Classic"),
        ];
        assert_eq!(categories(&presets), vec!["Classic", "custom", "Pro"]);
    }

    #[test]
    fn file_names_are_safe_and_nonempty() {
        assert_eq!(file_name_for("Tight Cross!"), "tight-cross.json");
        assert_eq!(file_name_for("  ../../etc  "), "etc.json");
        assert_eq!(file_name_for("***"), "crosshair.json");
    }
}
