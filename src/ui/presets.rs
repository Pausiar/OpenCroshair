//! Preset library page. Also hosts helpers shared with the editor and profiles pages: the file
//! path prompt (text field + Browse on Windows), preset categories, grid sizing and the
//! enabled-button helper.
use crate::crosshair::model::Preset;
use crate::state::{AppState, Page};
use crate::storage::import_export::{export_collection, export_crosshair, merge_imports, parse_import};
use crate::ui::theme::{self, Palette};
use crate::ui::widgets::{
    BUTTON_HEIGHT, Icon, PreviewScene, caption, danger_button, empty_state, ghost_button, paint_icon,
    preview_scene_box, primary_button, search_field, secondary_button,
};
use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};
use std::path::{Path, PathBuf};

const CARD_W: f32 = 176.0;
const CARD_H: f32 = 156.0;
const CARD_GAP: f32 = 16.0;
const CARD_RADIUS: u8 = 14;
const INPUT_RADIUS: u8 = 6;
/// Height of the thumbnail (the preview area inside a tile).
const THUMB_H: f32 = 92.0;
/// Height of the hover action row at the bottom of a tile.
const ACTION_ROW_H: f32 = 40.0;
const ACTION_SIZE: f32 = 28.0;
const ACTION_GAP: f32 = 6.0;
const STAR_SIZE: f32 = 24.0;

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

/// Number of columns for fixed-size items of width `item_w` with `gap` between them, in a row of
/// `avail` width: `max(1, floor((avail + gap) / (item_w + gap)))`.
pub(crate) fn grid_columns(avail: f32, item_w: f32, gap: f32) -> usize {
    let n = ((avail + gap) / (item_w + gap)).floor();
    // NaN and infinite widths fall back to a single column.
    if n.is_finite() { n.clamp(1.0, 64.0) as usize } else { 1 }
}

/// Shorten `text` to at most `max_chars` characters, ending in "..." when cut.
pub(crate) fn ellipsize(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{kept}...")
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

/// Button that is drawn in both states but only reports a click when enabled.
pub(crate) fn button_if(
    ui: &mut egui::Ui,
    enabled: bool,
    make: fn(&mut egui::Ui, &str) -> Response,
    label: &str,
) -> bool {
    let clicked = ui.add_enabled_ui(enabled, |ui| make(ui, label).clicked()).inner;
    enabled && clicked
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
    let pal = theme::palette(ctx);
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
            ui.label(egui::RichText::new(hint).size(12.0).color(pal.text_muted));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let width = if CAN_BROWSE { 300.0 } else { 420.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.path)
                        .hint_text("C:\\path\\to\\file.json")
                        .desired_width(width),
                );
                let browse = CAN_BROWSE && secondary_button(ui, "Browse...").clicked();
                if let Some(picked) = browse
                    .then(|| pick_path(prompt.kind, prompt.save, &prompt.path))
                    .flatten()
                {
                    prompt.path = picked.display().to_string();
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let ok = !prompt.path.trim().is_empty();
                let label = if prompt.save { "Save" } else { "Open" };
                if button_if(ui, ok, primary_button, label) {
                    outcome = PromptOutcome::Confirm(prompt.path.trim().to_string());
                }
                if secondary_button(ui, "Cancel").clicked() {
                    outcome = PromptOutcome::Cancel;
                }
            });
        });
    if !open && matches!(outcome, PromptOutcome::Open) {
        outcome = PromptOutcome::Cancel;
    }
    outcome
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Transfer {
    Import,
    Export,
}

/// Which presets the grid shows, besides the search text.
#[derive(Clone, Debug, Default, PartialEq)]
enum Filter {
    #[default]
    All,
    Favorites,
    Category(String),
}

fn matches_filter(p: &Preset, filter: &Filter) -> bool {
    match filter {
        Filter::All => true,
        Filter::Favorites => p.favorite,
        Filter::Category(c) => p.category.trim() == c,
    }
}

/// `query_lower` must already be trimmed and lowercased; an empty query matches everything.
fn matches_query(p: &Preset, query_lower: &str) -> bool {
    query_lower.is_empty()
        || p.name.to_lowercase().contains(query_lower)
        || p.category.to_lowercase().contains(query_lower)
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
    filter: Filter,
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

/// Hover actions on a tile. Built-in presets cannot be edited or deleted, so they get Open instead.
#[derive(Clone, Copy)]
enum TileOp {
    Apply,
    Open,
    Edit,
    Duplicate,
    Export,
    Delete,
}

fn tile_ops(builtin: bool) -> Vec<(TileOp, Icon, &'static str)> {
    let mut ops = vec![(TileOp::Apply, Icon::Check, "Apply")];
    if builtin {
        ops.push((TileOp::Open, Icon::Editor, "Open in editor"));
    } else {
        ops.push((TileOp::Edit, Icon::Edit, "Edit"));
    }
    ops.push((TileOp::Duplicate, Icon::Copy, "Duplicate"));
    ops.push((TileOp::Export, Icon::Download, "Export"));
    if !builtin {
        ops.push((TileOp::Delete, Icon::Trash, "Delete"));
    }
    ops
}

fn into_action(op: TileOp, id: String) -> Action {
    match op {
        TileOp::Apply => Action::Apply(id),
        TileOp::Open => Action::Open(id),
        TileOp::Edit => Action::Edit(id),
        TileOp::Duplicate => Action::Duplicate(id),
        TileOp::Export => Action::Export(id),
        TileOp::Delete => Action::Delete(id),
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let memory_id = ui.scope_id().with("presets_page_state");
    let mut pu: PresetsUi = ui.data(|d| d.get_temp(memory_id)).unwrap_or_default();

    toolbar(ui, state, &mut pu);
    ui.add_space(8.0);
    filter_chips(ui, state, &mut pu);
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
        ui.allocate_ui(Vec2::new(280.0, BUTTON_HEIGHT), |ui| {
            search_field(ui, &mut pu.query, "Search presets");
        });
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
        if button_if(ui, !state.presets.is_empty(), secondary_button, "Export all") {
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

/// Filter chips: All, Favorites, then one per category in the library.
fn filter_chips(ui: &mut egui::Ui, state: &AppState, pu: &mut PresetsUi) {
    let cats = categories(&state.presets);
    ui.horizontal_wrapped(|ui| {
        if chip(ui, "All", pu.filter == Filter::All) {
            pu.filter = Filter::All;
        }
        if chip(ui, "Favorites", pu.filter == Filter::Favorites) {
            pu.filter = Filter::Favorites;
        }
        for cat in cats {
            let selected = pu.filter == Filter::Category(cat.clone());
            if chip(ui, &cat, selected) {
                pu.filter = Filter::Category(cat);
            }
        }
    });
}

fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    if selected {
        primary_button(ui, label).clicked()
    } else {
        ghost_button(ui, label).clicked()
    }
}

fn grid(ui: &mut egui::Ui, state: &AppState, pu: &PresetsUi, actions: &mut Vec<Action>) {
    if state.presets.is_empty() {
        empty_state(
            ui,
            "No presets yet",
            "Save a crosshair from the editor or import a file.",
        );
        return;
    }
    let query = pu.query.trim().to_lowercase();
    let visible: Vec<&Preset> = state
        .presets
        .iter()
        .filter(|p| matches_filter(p, &pu.filter) && matches_query(p, &query))
        .collect();
    if visible.is_empty() {
        empty_state(ui, "No matches", "Change the search or the filter to see more presets.");
        return;
    }

    let active = state.settings.active_preset_id.as_deref();
    let cols = grid_columns(ui.available_width(), CARD_W, CARD_GAP);
    let row_gap = (CARD_GAP - ui.spacing().item_spacing.y).max(0.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for row in visible.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = CARD_GAP;
                for p in row {
                    card(ui, p, active == Some(p.id.as_str()), actions);
                }
            });
            ui.add_space(row_gap);
        }
    });
}

/// One preset tile. The tile is allocated once at a fixed size; everything inside is painted into
/// fixed rects, so hovering never changes the layout of the grid.
fn card(ui: &mut egui::Ui, p: &Preset, active: bool, actions: &mut Vec<Action>) {
    let pal = theme::palette(ui.ctx());
    let (tile, resp) = ui.allocate_exact_size(Vec2::new(CARD_W, CARD_H), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if !ui.is_rect_visible(tile) {
        return;
    }
    let hovered = ui.rect_contains_pointer(tile);
    let painter = ui.painter().clone();

    let border = if active {
        Stroke::new(2.0, pal.accent)
    } else if hovered {
        Stroke::new(1.0, pal.accent)
    } else {
        Stroke::new(1.0, pal.border)
    };
    painter.rect_filled(tile, CARD_RADIUS, pal.surface);
    painter.rect_stroke(tile, CARD_RADIUS, border, StrokeKind::Inside);

    // Dark preview thumbnail, scaled so the whole crosshair fits the 76 px square inside it.
    let thumb = Rect::from_min_size(tile.min + Vec2::splat(8.0), Vec2::new(CARD_W - 16.0, THUMB_H));
    let bounds = p.crosshair.bounding_size() as f32;
    preview_scene_box(
        ui,
        thumb,
        PreviewScene::Dark,
        &p.crosshair,
        (76.0 / bounds).clamp(0.1, 3.0),
    );

    let name_pos = Pos2::new(tile.left() + 12.0, thumb.bottom() + 10.0);
    painter.text(
        name_pos,
        Align2::LEFT_TOP,
        ellipsize(&p.name, 20),
        FontId::new(14.0, FontFamily::Name("semibold".into())),
        pal.text,
    );
    let meta_pos = Pos2::new(name_pos.x, name_pos.y + 22.0);
    let category = ellipsize(p.category.trim(), 12);
    let meta_font = FontId::proportional(12.0);
    let category_w = painter
        .layout_no_wrap(category.clone(), meta_font.clone(), pal.text_muted)
        .size()
        .x;
    painter.text(meta_pos, Align2::LEFT_TOP, &category, meta_font, pal.text_muted);
    if p.builtin {
        paint_badge(
            &painter,
            Pos2::new(meta_pos.x + category_w + 8.0, meta_pos.y),
            "Built-in",
            pal,
        );
    }

    // Favorite star on a small surface chip, top-right of the thumbnail. Its own click area.
    let star_c = Pos2::new(thumb.right() - 16.0, thumb.top() + 16.0);
    let star = ui
        .interact(
            Rect::from_center_size(star_c, Vec2::splat(STAR_SIZE)),
            ui.scope_id().with(("preset_fav", p.id.as_str())),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if p.favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        });
    let star_color = if p.favorite {
        pal.warning
    } else if star.hovered() {
        pal.text
    } else {
        pal.text_muted
    };
    painter.circle_filled(star_c, STAR_SIZE / 2.0, pal.surface);
    painter.circle_stroke(star_c, STAR_SIZE / 2.0, Stroke::new(1.0, pal.border));
    paint_icon(
        &painter,
        Rect::from_center_size(star_c, Vec2::splat(14.0)),
        Icon::Star { filled: p.favorite },
        star_color,
    );

    // Hover actions exist only while the pointer is over the tile, in a fixed bottom row.
    let ops = tile_ops(p.builtin);
    let clicked_op = if hovered {
        action_row(ui, &painter, tile, &p.id, &ops, pal)
    } else {
        None
    };
    if let Some(op) = clicked_op {
        actions.push(into_action(op, p.id.clone()));
    } else if star.clicked() {
        actions.push(Action::Favorite(p.id.clone()));
    } else if resp.clicked() {
        actions.push(Action::Apply(p.id.clone()));
    }
}

/// Small pill with the accent soft background, centered vertically on `pos`.
fn paint_badge(painter: &egui::Painter, pos: Pos2, text: &str, pal: &Palette) {
    let font = FontId::new(11.0, FontFamily::Name("semibold".into()));
    let text_w = painter
        .layout_no_wrap(text.to_owned(), font.clone(), pal.primary)
        .size()
        .x;
    let rect = Rect::from_min_size(Pos2::new(pos.x, pos.y - 3.0), Vec2::new(text_w + 12.0, 18.0));
    painter.rect_filled(rect, INPUT_RADIUS, pal.accent_soft);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, font, pal.primary);
}

/// Paints the action row at the bottom of `tile` and returns the operation clicked this frame.
fn action_row(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    tile: Rect,
    id: &str,
    ops: &[(TileOp, Icon, &'static str)],
    pal: &Palette,
) -> Option<TileOp> {
    let row = Rect::from_min_max(Pos2::new(tile.left(), tile.bottom() - ACTION_ROW_H), tile.max);
    painter.rect_filled(
        row,
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: CARD_RADIUS,
            se: CARD_RADIUS,
        },
        pal.surface_alt,
    );
    painter.line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, pal.border));

    let n = ops.len() as f32;
    let total = n * ACTION_SIZE + (n - 1.0) * ACTION_GAP;
    let mut x = tile.center().x - total / 2.0;
    let y = row.center().y - ACTION_SIZE / 2.0;
    let mut clicked = None;
    for (i, (op, icon, tip)) in ops.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(ACTION_SIZE));
        x += ACTION_SIZE + ACTION_GAP;
        let resp = ui
            .interact(r, ui.scope_id().with(("preset_op", id, i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let (fill, color) = if resp.hovered() {
            (pal.accent_soft, pal.primary)
        } else {
            (pal.surface, pal.text_muted)
        };
        painter.rect_filled(r, INPUT_RADIUS, fill);
        painter.rect_stroke(r, INPUT_RADIUS, Stroke::new(1.0, pal.border), StrokeKind::Inside);
        paint_icon(painter, r.shrink(6.0), *icon, color);
        if resp.clicked() && clicked.is_none() {
            clicked = Some(*op);
        }
    }
    clicked
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
            caption(ui, "Name");
            ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(340.0));
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
                if button_if(ui, valid, primary_button, "Save") {
                    save = true;
                }
                if secondary_button(ui, "Open in editor").clicked() {
                    open_editor = true;
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

    #[test]
    fn grid_columns_follow_the_formula() {
        // One card exactly fits; a second needs 2 * 176 + 16 = 368 px.
        assert_eq!(grid_columns(176.0, CARD_W, CARD_GAP), 1);
        assert_eq!(grid_columns(368.0, CARD_W, CARD_GAP), 2);
        assert_eq!(grid_columns(367.0, CARD_W, CARD_GAP), 1);
        assert_eq!(grid_columns(900.0, CARD_W, CARD_GAP), 4);
        assert_eq!(grid_columns(10.0, CARD_W, CARD_GAP), 1);
        assert_eq!(grid_columns(f32::NAN, CARD_W, CARD_GAP), 1);
    }

    #[test]
    fn ellipsize_keeps_short_text_and_cuts_long_text() {
        assert_eq!(ellipsize("Tight", 20), "Tight");
        assert_eq!(ellipsize("abcdefghij", 10), "abcdefghij");
        assert_eq!(ellipsize("abcdefghijk", 10), "abcdefg...");
        assert_eq!(ellipsize("abcdef", 2), "...");
    }

    #[test]
    fn filters_match_favorites_category_and_query() {
        let mut fav = preset("Glow", "Pro");
        fav.favorite = true;
        assert!(matches_filter(&fav, &Filter::Favorites));
        assert!(!matches_filter(&preset("Plain", "Pro"), &Filter::Favorites));
        assert!(matches_filter(
            &preset("Plain", " Pro "),
            &Filter::Category("Pro".into())
        ));
        assert!(!matches_filter(
            &preset("Plain", "Classic"),
            &Filter::Category("Pro".into())
        ));
        assert!(matches_query(&fav, ""));
        assert!(matches_query(&fav, "glo"));
        assert!(matches_query(&fav, "pr"));
        assert!(!matches_query(&fav, "zzz"));
    }

    #[test]
    fn built_in_tiles_offer_open_and_no_delete() {
        let builtin: Vec<TileOp> = tile_ops(true).into_iter().map(|(op, _, _)| op).collect();
        assert!(builtin.iter().any(|op| matches!(op, TileOp::Open)));
        assert!(!builtin.iter().any(|op| matches!(op, TileOp::Delete)));
        let custom: Vec<TileOp> = tile_ops(false).into_iter().map(|(op, _, _)| op).collect();
        assert!(custom.iter().any(|op| matches!(op, TileOp::Delete)));
        assert!(custom.iter().any(|op| matches!(op, TileOp::Edit)));
    }
}
