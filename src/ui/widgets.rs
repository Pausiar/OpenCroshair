//! Shared UI widgets: cards, buttons, toggles, segmented controls, sliders, tiles, vector icons
//! and the crosshair preview. Every color comes from the active [`theme::Palette`]
//! (`theme::palette(ui.ctx())`), except the preview scene backgrounds, which are fixed on purpose
//! so a crosshair is judged against the same background in both app themes.
//!
//! Layout rules shared by the buttons and tiles:
//! - Labels are centered horizontally and vertically; buttons are 32 px high and never wrap.
//! - Hover and press colors ease over [`ANIM_TIME`]; nothing repaints continuously.
//! - Hover actions are painted into a fixed rect (see [`tile_overlay_actions`]) so they never
//!   change the surrounding layout.
//! - Corner radii: inputs and chips 6, buttons 10, cards 14.
//!
//! Public API used by other pages (keep these signatures stable):
//!
//! ```text
//! pub fn paint_crosshair(ui: &mut egui::Ui, rect: egui::Rect, c: &Crosshair, zoom: f32)
//! pub fn preview_box(ui: &mut egui::Ui, rect: egui::Rect, dark: bool, c: &Crosshair, zoom: f32)
//! ```
//! `paint_crosshair` renders `c` with `crate::crosshair::renderer::render(c, zoom)` into a square
//! centered in `rect` (side = min(width, height)). The GPU texture is cached per (crosshair, zoom)
//! hash in egui temp memory, so calling it every frame for many presets is cheap.

// Library-style surface: some helpers are only used by pages that are migrating in parallel.
#![allow(dead_code)]

use crate::crosshair::model::Crosshair;
use crate::crosshair::renderer;
use crate::ui::theme::{self, Palette};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontFamily, FontId, Frame, Layout, Margin, Painter, Pos2, Rect, Response,
    RichText, Sense, Shadow, Shape, Stroke, StrokeKind, TextEdit, TextureHandle, TextureOptions, Ui, Vec2,
};
use egui::containers::collapsing_header::CollapsingState;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::RangeInclusive;

/// Hover and press transitions take this long (seconds).
pub const ANIM_TIME: f32 = 0.12;
/// Height of buttons, icon buttons, search fields and nav rows' hit area.
pub const BUTTON_HEIGHT: f32 = 32.0;
const BUTTON_MIN_WIDTH: f32 = 80.0;
const BUTTON_PAD_X: f32 = 16.0;
const CARD_RADIUS: u8 = 14;
const BUTTON_RADIUS: u8 = 10;
const INPUT_RADIUS: u8 = 6;

/// Scene backgrounds for the crosshair preview. Fixed colors, independent of the app theme.
pub const SCENE_DARK: Color32 = Color32::from_rgb(0x0B, 0x0F, 0x17);
pub const SCENE_GREY: Color32 = Color32::from_rgb(0x7A, 0x7F, 0x87);
pub const SCENE_LIGHT: Color32 = Color32::from_rgb(0xE5, 0xE7, 0xEB);
/// The two greys of the checkerboard scene.
pub const SCENE_CHECKER_A: Color32 = Color32::from_rgb(0x9C, 0xA1, 0xAA);
pub const SCENE_CHECKER_B: Color32 = Color32::from_rgb(0xD1, 0xD5, 0xDB);
const CHECKER_CELL: f32 = 12.0;

/// Stable egui id for a named piece of UI memory (fixed names only, never per-item).
pub fn state_id(name: &str) -> egui::Id {
    #[allow(deprecated)]
    egui::Id::new(name)
}

/// Serif heading text (Source Serif 4), used for page and card titles.
pub fn heading_text(text: &str, size: f32) -> RichText {
    RichText::new(text).family(FontFamily::Name("serif".into())).size(size)
}

/// Semibold UI text (Inter SemiBold).
pub fn semibold(text: &str) -> RichText {
    RichText::new(text).family(FontFamily::Name("semibold".into()))
}

/// Blend a color toward `b` by `t` (0..=1). Alpha-aware: a transparent endpoint fades in
/// instead of dragging the color toward black.
pub fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    match (a.a(), b.a()) {
        (0, 0) => Color32::TRANSPARENT,
        (0, _) => with_alpha_scale(b, t),
        (_, 0) => with_alpha_scale(a, 1.0 - t),
        _ => {
            let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
            Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
        }
    }
}

fn with_alpha_scale(c: Color32, k: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (f32::from(c.a()) * k).round() as u8)
}

/// Eased 0..=1 value for a hover state, using the shared transition time.
fn hover_t(ui: &Ui, id: egui::Id, hovered: bool) -> f32 {
    ui.ctx()
        .animate_value_with_time(id, if hovered { 1.0 } else { 0.0 }, ANIM_TIME)
}

/// Draw the crosshair preview into `rect` (see module docs).
pub fn paint_crosshair(ui: &mut Ui, rect: Rect, c: &Crosshair, zoom: f32) {
    let zoom = zoom.max(0.1);
    let ctx = ui.ctx().clone();
    let key = cache_key(c, zoom);
    let cache_id = state_id("oc_crosshair_texture_cache");

    let cached = ctx.data_mut(|d| {
        let cache: &mut HashMap<u64, TextureHandle> = d.get_temp_mut_or_default(cache_id);
        cache.get(&key).cloned()
    });
    let handle = match cached {
        Some(h) => h,
        None => {
            let bmp = renderer::render(c, zoom);
            let image =
                egui::ColorImage::from_rgba_premultiplied([bmp.width as usize, bmp.height as usize], &bmp.pixels);
            let h = ctx.load_texture(format!("crosshair_{key:x}"), image, TextureOptions::LINEAR);
            ctx.data_mut(|d| {
                let cache: &mut HashMap<u64, TextureHandle> = d.get_temp_mut_or_default(cache_id);
                if cache.len() >= 64 {
                    cache.clear();
                }
                cache.insert(key, h.clone());
            });
            h
        }
    };

    let side = rect.width().min(rect.height());
    let img = Rect::from_center_size(rect.center(), Vec2::splat(side));
    ui.painter().image(
        handle.id(),
        img,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn cache_key(c: &Crosshair, zoom: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(c).unwrap_or_default().hash(&mut h);
    zoom.to_bits().hash(&mut h);
    h.finish()
}

/// Background the crosshair preview is drawn on. Scene colors do not follow the app theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewScene {
    Dark,
    Grey,
    Light,
    Checker,
}

/// Solid background color of a scene. `Checker` returns its light cell color.
pub fn scene_color(scene: PreviewScene) -> Color32 {
    match scene {
        PreviewScene::Dark => SCENE_DARK,
        PreviewScene::Grey => SCENE_GREY,
        PreviewScene::Light => SCENE_LIGHT,
        PreviewScene::Checker => SCENE_CHECKER_B,
    }
}

/// Preview box with a fixed scene background and a crosshair centered inside `rect`.
pub fn preview_scene_box(ui: &mut Ui, rect: Rect, scene: PreviewScene, c: &Crosshair, zoom: f32) {
    let p = theme::palette(ui.ctx());
    let painter = ui.painter();
    painter.rect_filled(rect, INPUT_RADIUS, scene_color(scene));
    if scene == PreviewScene::Checker {
        let clipped = painter.with_clip_rect(rect);
        let cols = (rect.width() / CHECKER_CELL).ceil() as usize;
        let rows = (rect.height() / CHECKER_CELL).ceil() as usize;
        for row in 0..rows {
            for col in 0..cols {
                let cell = Rect::from_min_size(
                    rect.min + Vec2::new(col as f32 * CHECKER_CELL, row as f32 * CHECKER_CELL),
                    Vec2::splat(CHECKER_CELL),
                );
                let color = if (row + col) % 2 == 0 {
                    SCENE_CHECKER_A
                } else {
                    SCENE_CHECKER_B
                };
                clipped.rect_filled(cell, 0.0, color);
            }
        }
    }
    painter.rect_stroke(rect, INPUT_RADIUS, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint_crosshair(ui, rect.shrink(8.0), c, zoom);
}

/// Dark or light preview background (maps to [`PreviewScene::Dark`] / [`PreviewScene::Light`]).
pub fn preview_box(ui: &mut Ui, rect: Rect, dark: bool, c: &Crosshair, zoom: f32) {
    let scene = if dark { PreviewScene::Dark } else { PreviewScene::Light };
    preview_scene_box(ui, rect, scene, c, zoom);
}

fn card_shadow(dark: bool) -> Shadow {
    Shadow {
        offset: [0, 1],
        blur: 6,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 70 } else { 14 }),
    }
}

/// Card container: surface fill, 1px border, subtle shadow, radius 14, full available width.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    let dark = ui.visuals().dark_mode;
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CARD_RADIUS)
        .shadow(card_shadow(dark))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Card with a serif title and optional muted subtitle above the content.
pub fn titled_card<R>(ui: &mut Ui, title: &str, subtitle: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, |ui| {
        let p = theme::palette(ui.ctx());
        ui.label(heading_text(title, 17.0));
        if let Some(s) = subtitle {
            ui.label(RichText::new(s).size(13.0).color(p.text_muted));
        }
        ui.add_space(8.0);
        add(ui)
    })
}

/// Page title with a muted subtitle.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(heading_text(title, 26.0));
    ui.label(RichText::new(subtitle).size(14.0).color(p.text_muted));
    ui.add_space(16.0);
}

/// Small, faint, uppercase label used to group controls.
pub fn section_label(ui: &mut Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.add_space(4.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .family(FontFamily::Name("semibold".into()))
            .color(p.text_faint),
    );
}

/// Alias of [`section_label`], kept for existing callers.
pub fn section_header(ui: &mut Ui, text: &str) {
    section_label(ui, text);
}

/// Thin horizontal rule spanning the available width, with 12 px above and below.
pub fn divider(ui: &mut Ui) {
    let p = theme::palette(ui.ctx());
    ui.add_space(12.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.border);
    ui.add_space(12.0);
}

/// Centered placeholder for a list or page with nothing in it.
pub fn empty_state(ui: &mut Ui, title: &str, body: &str) {
    let p = theme::palette(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(32.0);
        ui.label(heading_text(title, 18.0));
        ui.add_space(4.0);
        ui.label(RichText::new(body).size(13.0).color(p.text_muted));
        ui.add_space(32.0);
    });
}

/// Small rounded tag in the accent colors, e.g. "New" or "Default".
pub fn badge(ui: &mut Ui, text: &str) -> Response {
    let p = theme::palette(ui.ctx());
    Frame::new()
        .fill(p.accent_soft)
        .corner_radius(INPUT_RADIUS)
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).strong().color(p.primary));
        })
        .response
}

/// Round swatch for picking a color. `selected` draws a ring around it.
pub fn color_swatch(ui: &mut Ui, color: Color32, selected: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();
        if selected {
            painter.circle_stroke(center, 11.5, Stroke::new(2.0, p.primary));
        }
        painter.circle_filled(center, 8.5, color);
        painter.circle_stroke(center, 8.5, Stroke::new(1.0, p.border));
    }
    resp
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonKind {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ButtonColors {
    fill: Color32,
    stroke: Color32,
    text: Color32,
}

/// Colors for a button at hover progress `hover` (0 = rest, 1 = hovered) and press state.
fn button_colors(p: &Palette, kind: ButtonKind, hover: f32, pressed: bool) -> ButtonColors {
    let (rest_fill, hover_fill, rest_stroke, hover_stroke, text) = match kind {
        ButtonKind::Primary => (p.primary, p.primary_hover, p.primary, p.primary_hover, p.on_primary),
        ButtonKind::Secondary => (p.surface, p.surface_alt, p.border, p.accent, p.text),
        ButtonKind::Danger => (
            p.surface,
            p.surface_alt,
            p.danger.gamma_multiply(0.6),
            p.danger,
            p.danger,
        ),
        ButtonKind::Ghost => (
            Color32::TRANSPARENT,
            p.surface_alt,
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            p.text_muted,
        ),
    };
    let mut fill = mix_color(rest_fill, hover_fill, hover);
    if pressed {
        fill = mix_color(fill, text, 0.12);
    }
    let text = match kind {
        ButtonKind::Ghost => mix_color(p.text_muted, p.text, hover),
        _ => text,
    };
    ButtonColors {
        fill,
        stroke: mix_color(rest_stroke, hover_stroke, hover),
        text,
    }
}

/// Shared button body: fixed 32 px height, minimum width, centered single-line label.
fn button(ui: &mut Ui, text: &str, kind: ButtonKind) -> Response {
    let p = theme::palette(ui.ctx());
    let font = match kind {
        ButtonKind::Primary => FontId::new(14.0, FontFamily::Name("semibold".into())),
        _ => FontId::new(14.0, FontFamily::Proportional),
    };
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), p.text)
        .size()
        .x;
    let size = Vec2::new((text_w + 2.0 * BUTTON_PAD_X).max(BUTTON_MIN_WIDTH), BUTTON_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let c = button_colors(p, kind, hover, resp.is_pointer_button_down_on());
        let enabled = ui.is_enabled();
        let text_color = if enabled { c.text } else { c.text.gamma_multiply(0.45) };
        let painter = ui.painter();
        painter.rect_filled(rect, BUTTON_RADIUS, c.fill);
        painter.rect_stroke(rect, BUTTON_RADIUS, Stroke::new(1.0, c.stroke), StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, text, font, text_color);
    }
    resp
}

/// Primary (dark blue in light mode, pale blue in dark mode) button.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, ButtonKind::Primary)
}

/// Neutral surface button.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, ButtonKind::Secondary)
}

/// Destructive button (red text, neutral surface).
pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, ButtonKind::Danger)
}

/// Text-only button without a resting background; shows a surface on hover.
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    button(ui, text, ButtonKind::Ghost)
}

/// Square 32 px icon-only button. The tooltip is shown on hover.
pub fn icon_button(ui: &mut Ui, kind: Icon, tooltip: &str) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BUTTON_HEIGHT), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tooltip);
    if ui.is_rect_visible(rect) {
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            BUTTON_RADIUS,
            mix_color(Color32::TRANSPARENT, p.surface_alt, hover),
        );
        let color = mix_color(p.text_muted, p.text, hover);
        paint_icon(
            painter,
            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
            kind,
            color,
        );
    }
    resp
}

/// Animated on/off switch. Sets `on` and returns a response whose `changed()` is true on click.
pub fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    let p = theme::palette(ui.ctx());
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let radius = rect.height() / 2.0;
        painter.rect_filled(rect, radius, lerp_color(p.surface_alt, p.primary, t));
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, lerp_color(p.border, p.primary, t)),
            StrokeKind::Inside,
        );
        let knob_r = radius - 3.0;
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        painter.circle_filled(Pos2::new(x, rect.center().y), knob_r, p.surface);
    }
    resp
}

/// Label + optional description on the left, switch on the right. Returns true when toggled.
pub fn toggle_row(ui: &mut Ui, label: &str, description: Option<&str>, on: &mut bool) -> bool {
    let p = theme::palette(ui.ctx());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(14.0).color(p.text));
            if let Some(d) = description {
                ui.label(RichText::new(d).size(13.0).color(p.text_muted));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = toggle_switch(ui, on).changed();
        });
    });
    changed
}

/// Label, slider and compact value box on one aligned row. Returns true when the value changed.
/// `step` is the slider and drag increment; `suffix` is shown after the value (e.g. "px").
pub fn slider_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    suffix: &str,
    step: f32,
) -> bool {
    const LABEL_W: f32 = 132.0;
    const VALUE_W: f32 = 76.0;
    const ROW_H: f32 = 28.0;
    let p = theme::palette(ui.ctx());
    let step = step.max(f32::EPSILON);
    let decimals = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else {
        2
    };
    let spacing = ui.spacing().item_spacing.x;
    let slider_w = (ui.available_width() - LABEL_W - VALUE_W - 2.0 * spacing).max(60.0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(LABEL_W, ROW_H), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(label).size(14.0).color(p.text));
        });
        let slider = ui.add_sized(
            [slider_w, ROW_H],
            egui::Slider::new(value, range.clone())
                .show_value(false)
                .step_by(f64::from(step)),
        );
        let drag = ui.add_sized(
            [VALUE_W, ROW_H],
            egui::DragValue::new(value)
                .range(range)
                .speed(step)
                .suffix(suffix)
                .max_decimals(decimals),
        );
        changed = slider.changed() || drag.changed();
    });
    changed
}

/// Segmented control with an animated sliding pill. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    const PAD_X: f32 = 14.0;
    const INSET: f32 = 3.0;
    const GAP: f32 = 2.0;
    const CELL_H: f32 = 28.0;
    const MIN_CELL_W: f32 = 56.0;
    let p = theme::palette(ui.ctx());
    let font = FontId::new(13.0, FontFamily::Proportional);
    let label_w = options
        .iter()
        .map(|(_, l)| {
            ui.painter()
                .layout_no_wrap((*l).to_owned(), font.clone(), p.text)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let cell_w = (label_w + 2.0 * PAD_X).max(MIN_CELL_W);
    let n = options.len().max(1) as f32;
    let size = Vec2::new(n * cell_w + (n - 1.0) * GAP + 2.0 * INSET, CELL_H + 2.0 * INSET);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let cell_rect = |i: usize| {
        Rect::from_min_size(
            Pos2::new(rect.left() + INSET + i as f32 * (cell_w + GAP), rect.top() + INSET),
            Vec2::new(cell_w, CELL_H),
        )
    };

    let active = options.iter().position(|(v, _)| *v == *current).unwrap_or(0);
    let pill_pos = ui
        .ctx()
        .animate_value_with_time(resp.id.with("pill"), active as f32, ANIM_TIME);

    let mut clicked = None;
    let mut hovered = vec![false; options.len()];
    for (i, (value, _)) in options.iter().enumerate() {
        let r = ui
            .interact(cell_rect(i), resp.id.with(("seg", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        hovered[i] = r.hovered();
        if r.clicked() && *value != *current {
            clicked = Some(*value);
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, INPUT_RADIUS, p.surface_alt);
        painter.rect_stroke(rect, INPUT_RADIUS, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let pill = Rect::from_min_size(
            Pos2::new(rect.left() + INSET + pill_pos * (cell_w + GAP), rect.top() + INSET),
            Vec2::new(cell_w, CELL_H),
        );
        painter.rect_filled(pill, INPUT_RADIUS, p.primary);
        for (i, (_, label)) in options.iter().enumerate() {
            let closeness = 1.0 - (pill_pos - i as f32).abs().min(1.0);
            let base = if hovered[i] { p.text } else { p.text_muted };
            let color = lerp_color(base, p.on_primary, closeness);
            painter.text(cell_rect(i).center(), Align2::CENTER_CENTER, label, font.clone(), color);
        }
    }

    match clicked {
        Some(v) => {
            *current = v;
            true
        }
        None => false,
    }
}

/// Monospace key cap, used for hotkey strings.
pub fn key_chip(ui: &mut Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    Frame::new()
        .fill(p.surface_alt)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(INPUT_RADIUS)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::new(12.0, FontFamily::Monospace))
                    .color(p.text),
            );
        });
}

/// Rounded colored label, e.g. "Active" / "Off".
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.16))
        .corner_radius(99.0)
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).strong().color(color));
        });
}

/// Single-line search input with a leading magnifier. Returns the text field's response.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    let p = theme::palette(ui.ctx());
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(INPUT_RADIUS)
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, Icon::Search, 16.0, p.text_faint);
                let width = ui.available_width();
                ui.add(
                    TextEdit::singleline(text)
                        .hint_text(hint)
                        .frame(Frame::NONE)
                        .desired_width(width),
                )
            })
            .inner
        })
        .inner
}

/// Tile for a shape picker: a preview area painted by `paint`, plus a caption.
/// The selected tile gets a filled accent background and a 2 px border.
pub fn shape_tile(ui: &mut Ui, selected: bool, label: &str, paint: impl FnOnce(&Painter, Rect)) -> Response {
    const W: f32 = 96.0;
    const H: f32 = 96.0;
    const PREVIEW_H: f32 = 56.0;
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(W, H), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let (fill, border, border_w) = if selected {
            (p.accent_soft, p.accent, 2.0)
        } else {
            (
                mix_color(p.surface, p.surface_alt, hover),
                mix_color(p.border, p.accent, hover),
                1.0,
            )
        };
        let preview = Rect::from_min_size(rect.min + Vec2::splat(8.0), Vec2::new(W - 16.0, PREVIEW_H));
        let painter = ui.painter();
        painter.rect_filled(rect, BUTTON_RADIUS, fill);
        painter.rect_stroke(rect, BUTTON_RADIUS, Stroke::new(border_w, border), StrokeKind::Inside);
        painter.rect_filled(preview, INPUT_RADIUS, p.surface_alt);
        paint(&painter.with_clip_rect(preview), preview);
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 12.0),
            Align2::CENTER_CENTER,
            label,
            FontId::new(12.0, FontFamily::Proportional),
            if selected { p.primary } else { p.text_muted },
        );
    }
    resp
}

const OVERLAY_SIZE: f32 = 26.0;
const OVERLAY_MARGIN: f32 = 6.0;
const OVERLAY_GAP: f32 = 4.0;

/// Rect of the `index`-th hover action inside `tile`. Slot 0 is the rightmost, at the top.
/// Always inside the tile, so the actions never extend the layout.
pub fn overlay_slot(tile: Rect, index: usize) -> Rect {
    let x = tile.right() - OVERLAY_MARGIN - (index as f32 + 1.0) * OVERLAY_SIZE - index as f32 * OVERLAY_GAP;
    Rect::from_min_size(Pos2::new(x, tile.top() + OVERLAY_MARGIN), Vec2::splat(OVERLAY_SIZE))
}

/// Hover actions for a tile (for example edit / duplicate / delete). They are painted into fixed
/// slots at the tile's top-right corner, only while the pointer is over `tile`. Returns the index
/// of the action clicked this frame, if any. Nothing is allocated in the layout.
pub fn tile_overlay_actions(ui: &mut Ui, tile: Rect, actions: &[(Icon, &str)]) -> Option<usize> {
    if !ui.rect_contains_pointer(tile) {
        return None;
    }
    let p = theme::palette(ui.ctx());
    let mut clicked = None;
    for (i, (kind, tip)) in actions.iter().enumerate() {
        let slot = overlay_slot(tile, i);
        let id = ui
            .scope_id()
            .with(("oc_tile_action", tile.min.x.to_bits(), tile.min.y.to_bits(), i));
        let resp = ui
            .interact(slot, id, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let painter = ui.painter();
        painter.rect_filled(slot, INPUT_RADIUS, mix_color(p.surface, p.surface_alt, hover));
        painter.rect_stroke(slot, INPUT_RADIUS, Stroke::new(1.0, p.border), StrokeKind::Inside);
        paint_icon(
            painter,
            Rect::from_center_size(slot.center(), Vec2::splat(14.0)),
            *kind,
            mix_color(p.text_muted, p.text, hover),
        );
        if resp.clicked() && clicked.is_none() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Sidebar-style navigation row: 36 px, icon, label, and an accent bar when selected.
pub fn nav_item(ui: &mut Ui, kind: Icon, label: &str, selected: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let bg = if selected {
            p.accent_soft
        } else {
            mix_color(Color32::TRANSPARENT, p.surface_alt, hover)
        };
        let fg = if selected {
            p.primary
        } else {
            mix_color(p.text_muted, p.text, hover)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, INPUT_RADIUS, bg);
        if selected {
            let bar = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 8.0), Vec2::new(3.0, 20.0));
            painter.rect_filled(bar, 2.0, p.accent);
        }
        let icon_rect = Rect::from_min_size(rect.left_top() + Vec2::new(12.0, 8.0), Vec2::splat(20.0));
        paint_icon(painter, icon_rect, kind, fg);
        painter.text(
            rect.left_top() + Vec2::new(42.0, 18.0),
            Align2::LEFT_CENTER,
            label,
            FontId::new(14.0, FontFamily::Proportional),
            fg,
        );
    }
    resp
}

/// Collapsible section with its own 32 px header (animated chevron). Returns the body's result
/// while open (and while it is animating closed), otherwise `None`.
/// `id` must be unique among the sections of the same parent `ui`.
pub fn collapsing_section<R>(
    ui: &mut Ui,
    id: &str,
    title: &str,
    default_open: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let p = theme::palette(ui.ctx());
    let id = ui.scope_id().with(("oc_section", id));
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if resp.clicked() {
        state.toggle(ui);
    }
    let openness = state.openness(ui.ctx());
    if ui.is_rect_visible(rect) {
        let hover = hover_t(ui, resp.id.with("hover"), resp.hovered());
        let painter = ui.painter();
        let chevron_center = rect.left_center() + Vec2::new(8.0, 0.0);
        paint_chevron(
            painter,
            chevron_center,
            openness,
            mix_color(p.text_muted, p.text, hover),
        );
        painter.text(
            rect.left_center() + Vec2::new(24.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            FontId::new(14.0, FontFamily::Name("semibold".into())),
            p.text,
        );
    }
    let body = state.show_body_unindented(ui, add).map(|r| r.inner);
    state.store(ui.ctx());
    body
}

/// Chevron that rotates from right-pointing (`t` = 0) to down-pointing (`t` = 1).
fn paint_chevron(painter: &Painter, center: Pos2, t: f32, color: Color32) {
    let angle = t.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    let (s, c) = angle.sin_cos();
    let rot = |x: f32, y: f32| center + Vec2::new(x * c - y * s, x * s + y * c);
    let pts = vec![rot(-2.5, -4.5), rot(2.5, 0.0), rot(-2.5, 4.5)];
    painter.add(Shape::line(pts, Stroke::new(1.8, color)));
}

/// Small uppercase-free text with the faint color, for captions under controls.
pub fn caption(ui: &mut Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(text).size(12.0).color(p.text_faint));
}

/// Vector icons, drawn on a 24x24 grid (stroke style, scaled to the requested size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Overview,
    Editor,
    Presets,
    Profiles,
    Settings,
    Power,
    Eye,
    Plus,
    Minimize,
    Maximize,
    Restore,
    Close,
    Sun,
    Moon,
    /// Five-point star; `filled` fills the shape.
    Star {
        filled: bool,
    },
    Check,
    Search,
    /// Chevron pointing down (`down: true`) or right.
    Chevron {
        down: bool,
    },
    Copy,
    Trash,
    Edit,
    Download,
    Upload,
}

/// Paint an icon into a `size` x `size` box (allocated in the layout).
pub fn icon(ui: &mut Ui, kind: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_icon(ui.painter(), rect, kind, color);
    }
    resp
}

/// Paint an icon directly with a painter inside `rect` (used by clickable items).
pub fn paint_icon(p: &Painter, rect: Rect, kind: Icon, color: Color32) {
    let k = rect.width().min(rect.height()) / 24.0;
    let origin = Rect::from_center_size(rect.center(), Vec2::splat(rect.width().min(rect.height()))).min;
    let to = |x: f32, y: f32| origin + Vec2::new(x * k, y * k);
    let stroke = Stroke::new(1.8 * k, color);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| to(x, y)).collect(), stroke));
    };
    let seg = |a: (f32, f32), b: (f32, f32)| line(&[a, b]);
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(
            Rect::from_min_max(to(x0, y0), to(x1, y1)),
            2.0 * k,
            stroke,
            StrokeKind::Middle,
        );
    };
    match kind {
        Icon::Overview => {
            rr(3.0, 3.0, 10.0, 10.0);
            rr(14.0, 3.0, 21.0, 10.0);
            rr(3.0, 14.0, 10.0, 21.0);
            rr(14.0, 14.0, 21.0, 21.0);
        }
        Icon::Editor => {
            p.circle_stroke(to(12.0, 12.0), 7.0 * k, stroke);
            seg((12.0, 2.0), (12.0, 7.0));
            seg((12.0, 17.0), (12.0, 22.0));
            seg((2.0, 12.0), (7.0, 12.0));
            seg((17.0, 12.0), (22.0, 12.0));
            p.circle_filled(to(12.0, 12.0), 1.4 * k, color);
        }
        Icon::Presets => {
            line(&[(12.0, 3.0), (21.0, 8.0), (12.0, 13.0), (3.0, 8.0), (12.0, 3.0)]);
            line(&[(3.0, 13.0), (12.0, 18.0), (21.0, 13.0)]);
            line(&[(3.0, 17.5), (12.0, 22.5), (21.0, 17.5)]);
        }
        Icon::Profiles => {
            rr(2.5, 7.0, 21.5, 17.5);
            seg((6.5, 12.0), (10.5, 12.0));
            seg((8.5, 10.0), (8.5, 14.0));
            p.circle_filled(to(16.0, 11.0), 1.2 * k, color);
            p.circle_filled(to(18.0, 13.5), 1.2 * k, color);
        }
        Icon::Settings => {
            seg((3.0, 6.0), (21.0, 6.0));
            seg((3.0, 12.0), (21.0, 12.0));
            seg((3.0, 18.0), (21.0, 18.0));
            p.circle_filled(to(9.0, 6.0), 2.6 * k, color);
            p.circle_filled(to(16.0, 12.0), 2.6 * k, color);
            p.circle_filled(to(7.0, 18.0), 2.6 * k, color);
        }
        Icon::Power => {
            let pts: Vec<(f32, f32)> = (0..=40)
                .map(|i| {
                    let a = (-50.0 + 280.0 * i as f32 / 40.0).to_radians();
                    (12.0 + 8.5 * a.cos(), 13.0 + 8.5 * a.sin())
                })
                .collect();
            line(&pts);
            seg((12.0, 3.0), (12.0, 11.5));
        }
        Icon::Eye => {
            let pts: Vec<(f32, f32)> = (0..=36)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / 36.0;
                    (12.0 + 10.0 * t.cos(), 12.0 + 6.5 * t.sin())
                })
                .collect();
            line(&pts);
            p.circle_stroke(to(12.0, 12.0), 3.0 * k, stroke);
        }
        Icon::Plus => {
            seg((12.0, 5.0), (12.0, 19.0));
            seg((5.0, 12.0), (19.0, 12.0));
        }
        Icon::Minimize => seg((5.0, 12.0), (19.0, 12.0)),
        Icon::Maximize => rr(5.0, 5.0, 19.0, 19.0),
        Icon::Restore => {
            rr(4.0, 8.0, 16.0, 20.0);
            line(&[(8.0, 8.0), (8.0, 4.0), (20.0, 4.0), (20.0, 16.0), (16.0, 16.0)]);
        }
        Icon::Close => {
            seg((6.0, 6.0), (18.0, 18.0));
            seg((18.0, 6.0), (6.0, 18.0));
        }
        Icon::Sun => {
            p.circle_stroke(to(12.0, 12.0), 4.0 * k, stroke);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let (s, c) = a.sin_cos();
                seg((12.0 + 7.0 * c, 12.0 + 7.0 * s), (12.0 + 9.5 * c, 12.0 + 9.5 * s));
            }
        }
        Icon::Moon => {
            // Outer arc through the left side, then an inner curve back to the start point.
            let mut pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let a = (290.0 - 220.0 * i as f32 / 24.0).to_radians();
                    (12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin())
                })
                .collect();
            let (start, end) = (pts[0], pts[pts.len() - 1]);
            for i in 1..=12 {
                let t = i as f32 / 12.0;
                let x = (1.0 - t).powi(2) * end.0 + 2.0 * (1.0 - t) * t * 7.0 + t * t * start.0;
                let y = (1.0 - t).powi(2) * end.1 + 2.0 * (1.0 - t) * t * 12.0 + t * t * start.1;
                pts.push((x, y));
            }
            line(&pts);
        }
        Icon::Star { filled } => {
            let center = (12.0, 12.8);
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 9.5 } else { 4.2 };
                    let a = (-90.0 + 36.0 * i as f32).to_radians();
                    to(center.0 + r * a.cos(), center.1 + r * a.sin())
                })
                .collect();
            if filled {
                p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
            } else {
                let mut closed = pts;
                closed.push(closed[0]);
                p.add(Shape::line(closed, stroke));
            }
        }
        Icon::Check => line(&[(4.0, 12.5), (9.5, 18.0), (20.0, 6.5)]),
        Icon::Search => {
            p.circle_stroke(to(10.5, 10.5), 6.5 * k, stroke);
            seg((15.5, 15.5), (21.0, 21.0));
        }
        Icon::Chevron { down: true } => line(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
        Icon::Chevron { down: false } => line(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)]),
        Icon::Copy => {
            rr(3.0, 3.0, 16.0, 16.0);
            rr(8.0, 8.0, 21.0, 21.0);
        }
        Icon::Trash => {
            seg((4.0, 6.0), (20.0, 6.0));
            seg((9.0, 3.0), (15.0, 3.0));
            rr(6.0, 6.0, 18.0, 21.0);
            seg((10.0, 10.0), (10.0, 17.0));
            seg((14.0, 10.0), (14.0, 17.0));
        }
        Icon::Edit => line(&[
            (15.0, 5.0),
            (19.0, 9.0),
            (9.0, 19.0),
            (5.0, 19.0),
            (5.0, 15.0),
            (15.0, 5.0),
        ]),
        Icon::Download => {
            seg((12.0, 3.0), (12.0, 15.0));
            line(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)]);
            line(&[(4.0, 15.0), (4.0, 20.0), (20.0, 20.0), (20.0, 15.0)]);
        }
        Icon::Upload => {
            seg((12.0, 15.0), (12.0, 3.0));
            line(&[(7.0, 8.0), (12.0, 3.0), (17.0, 8.0)]);
            line(&[(4.0, 15.0), (4.0, 20.0), (20.0, 20.0), (20.0, 15.0)]);
        }
    }
}

/// Linear interpolation between two opaque colors (t in 0..=1).
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::config::ThemeMode;
    use crate::ui::{fonts, theme};
    use eframe::egui::{Pos2, RawInput, Vec2};

    #[test]
    fn lerp_endpoints() {
        let a = Color32::from_rgb(10, 20, 30);
        let b = Color32::from_rgb(200, 210, 220);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 2.0), b);
    }

    #[test]
    fn mix_fades_transparent_endpoint_without_darkening() {
        let c = Color32::from_rgb(200, 100, 50);
        let half = mix_color(Color32::TRANSPARENT, c, 0.5).to_srgba_unmultiplied();
        // Premultiplied storage rounds by about one unit at half alpha.
        for (got, want) in half[..3].iter().zip([200i32, 100, 50]) {
            assert!((i32::from(*got) - want).abs() <= 2, "channel {got} vs {want}");
        }
        assert_eq!(half[3], 128);
        assert_eq!(mix_color(Color32::TRANSPARENT, c, 1.0), c);
        assert_eq!(
            mix_color(Color32::TRANSPARENT, Color32::TRANSPARENT, 0.5),
            Color32::TRANSPARENT
        );
    }

    #[test]
    fn cache_key_changes_with_inputs() {
        let a = Crosshair::default();
        let mut b = Crosshair::default();
        b.gap += 1.0;
        assert_eq!(cache_key(&a, 4.0), cache_key(&a, 4.0));
        assert_ne!(cache_key(&a, 4.0), cache_key(&b, 4.0));
        assert_ne!(cache_key(&a, 4.0), cache_key(&a, 2.0));
    }

    #[test]
    fn scene_colors_are_fixed() {
        assert_eq!(scene_color(PreviewScene::Dark), Color32::from_rgb(0x0B, 0x0F, 0x17));
        assert_eq!(scene_color(PreviewScene::Grey), Color32::from_rgb(0x7A, 0x7F, 0x87));
        assert_eq!(scene_color(PreviewScene::Light), Color32::from_rgb(0xE5, 0xE7, 0xEB));
        assert_ne!(SCENE_CHECKER_A, SCENE_CHECKER_B);
    }

    #[test]
    fn button_colors_rest_and_hover_match_palette() {
        let p = &theme::LIGHT;
        let rest = button_colors(p, ButtonKind::Secondary, 0.0, false);
        assert_eq!(rest.fill, p.surface);
        assert_eq!(rest.stroke, p.border);
        let hover = button_colors(p, ButtonKind::Secondary, 1.0, false);
        assert_eq!(hover.fill, p.surface_alt);
        assert_eq!(hover.stroke, p.accent);
        let primary = button_colors(&theme::DARK, ButtonKind::Primary, 0.0, false);
        assert_eq!(primary.text, theme::DARK.on_primary);
        assert_eq!(
            button_colors(p, ButtonKind::Ghost, 0.0, false).fill,
            Color32::TRANSPARENT
        );
    }

    #[test]
    fn overlay_slots_stay_inside_tile() {
        let tile = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(120.0, 100.0));
        for i in 0..3 {
            let slot = overlay_slot(tile, i);
            assert!(tile.contains_rect(slot), "slot {i} escapes the tile");
        }
        assert!(overlay_slot(tile, 0).left() > overlay_slot(tile, 1).left());
    }

    const ALL_ICONS: [Icon; 26] = [
        Icon::Overview,
        Icon::Editor,
        Icon::Presets,
        Icon::Profiles,
        Icon::Settings,
        Icon::Power,
        Icon::Eye,
        Icon::Plus,
        Icon::Minimize,
        Icon::Maximize,
        Icon::Restore,
        Icon::Close,
        Icon::Sun,
        Icon::Moon,
        Icon::Star { filled: true },
        Icon::Star { filled: false },
        Icon::Check,
        Icon::Search,
        Icon::Chevron { down: true },
        Icon::Chevron { down: false },
        Icon::Copy,
        Icon::Trash,
        Icon::Edit,
        Icon::Download,
        Icon::Upload,
        Icon::Editor,
    ];

    /// Drops a frame's output after clearing its texture deltas (egui panics on unapplied deltas).
    fn discard(mut out: egui::FullOutput) {
        out.textures_delta.clear();
    }

    /// Runs `body` for one frame in the given theme, after one warm-up frame so the
    /// fonts (installed in the first frame) are ready.
    fn run_frame(mode: ThemeMode, mut body: impl FnMut(&mut Ui)) {
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        theme::apply(&ctx, mode);
        let raw = || RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
            ..Default::default()
        };
        discard(ctx.run_ui(raw(), |_| {}));
        discard(ctx.run_ui(raw(), |ui| body(ui)));
    }

    #[test]
    fn smoke_all_widgets_in_both_themes() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            run_frame(mode, |ui| {
                let p = theme::palette(ui.ctx());
                let c = Crosshair::default();
                let mut on = true;
                let mut tab = 0usize;
                let mut query = String::from("abc");
                let mut value = 4.0_f32;

                card(ui, |ui| ui.label("card"));
                titled_card(ui, "Title", Some("sub"), |ui| ui.label("body"));
                page_header(ui, "Page", "Subtitle");
                section_label(ui, "section");
                divider(ui);
                empty_state(ui, "Nothing yet", "Add one to get started.");
                badge(ui, "New");
                color_swatch(ui, p.accent, true);
                color_swatch(ui, p.accent, false);
                primary_button(ui, "Primary");
                secondary_button(ui, "Secondary");
                danger_button(ui, "Danger");
                ghost_button(ui, "Ghost");
                icon_button(ui, Icon::Plus, "Add");
                toggle_switch(ui, &mut on);
                toggle_row(ui, "Row", Some("description"), &mut on);
                segmented(ui, &mut tab, &[(0usize, "One"), (1, "Two")]);
                key_chip(ui, "Ctrl");
                status_pill(ui, "Active", p.success);
                slider_row(ui, "Size", &mut value, 0.0..=10.0, "px", 0.5);
                search_field(ui, &mut query, "Search");
                nav_item(ui, Icon::Overview, "Overview", true);
                nav_item(ui, Icon::Settings, "Settings", false);
                let _ = collapsing_section(ui, "smoke", "Section", true, |ui| ui.label("inside"));
                let tile = shape_tile(ui, true, "Circle", |painter, r| {
                    painter.circle_filled(r.center(), 10.0, p.accent);
                });
                tile_overlay_actions(ui, tile.rect, &[(Icon::Edit, "Edit"), (Icon::Trash, "Delete")]);
                let (preview, _) = ui.allocate_exact_size(Vec2::new(180.0, 180.0), Sense::hover());
                preview_box(ui, preview, true, &c, 1.0);
                preview_scene_box(ui, preview, PreviewScene::Checker, &c, 1.0);
                preview_scene_box(ui, preview, PreviewScene::Grey, &c, 1.0);
                paint_crosshair(ui, preview, &c, 1.0);
                caption(ui, "caption");
                ui.label(heading_text("Heading", 18.0));
                ui.label(semibold("Semibold"));
                for kind in ALL_ICONS {
                    icon(ui, kind, 16.0, p.text);
                }
            });
        }
    }

    #[test]
    fn collapsing_section_renders_closed_without_body() {
        run_frame(ThemeMode::Light, |ui| {
            let body = collapsing_section(ui, "closed", "Closed", false, |ui| ui.label("hidden"));
            assert!(body.is_none());
        });
    }
}
