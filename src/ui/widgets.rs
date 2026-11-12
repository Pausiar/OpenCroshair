//! Shared UI widgets: cards, buttons, toggles, segmented controls, vector icons and the
//! crosshair preview. No emoji and no image files; icons are drawn with egui painter paths.
//!
//! Public API used by other pages (keep these signatures stable):
//!
//! ```text
//! pub fn paint_crosshair(ui: &mut egui::Ui, rect: egui::Rect, c: &Crosshair, zoom: f32)
//! ```
//! Renders `c` with `crate::crosshair::renderer::render(c, zoom)` into a square centered in
//! `rect` (side = min(width, height)). The GPU texture is cached per (crosshair, zoom) hash in
//! egui temp memory, so calling it every frame for many presets is cheap.
use crate::crosshair::model::Crosshair;
use crate::crosshair::renderer;
use crate::ui::theme::{self, BLUE, BLUE_LIGHT, BORDER, PREVIEW_DARK, PREVIEW_LIGHT, SURFACE, SURFACE2, TEXT, TEXT2};
use eframe::egui::{
    self, Align, Color32, CursorIcon, FontFamily, FontId, Layout, Margin, Pos2, Rect, Response, RichText, Sense, Shape,
    Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Stable egui id for a named piece of UI memory (fixed names only, never per-item).
pub fn state_id(name: &str) -> egui::Id {
    #[allow(deprecated)]
    egui::Id::new(name)
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

/// Filled preview background (dark or light) with a crosshair drawn centered inside `rect`.
pub fn preview_box(ui: &mut Ui, rect: Rect, dark: bool, c: &Crosshair, zoom: f32) {
    let bg = if dark { PREVIEW_DARK } else { PREVIEW_LIGHT };
    ui.painter().rect_filled(rect, theme::RADIUS_SM, bg);
    ui.painter()
        .rect_stroke(rect, theme::RADIUS_SM, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    paint_crosshair(ui, rect.shrink(8.0), c, zoom);
}

/// Card container: surface fill, subtle border, rounded corners, full available width.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Card with a bold title and optional muted subtitle above the content.
pub fn titled_card<R>(ui: &mut Ui, title: &str, subtitle: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, |ui| {
        ui.label(RichText::new(title).size(15.0).strong().color(TEXT));
        if let Some(s) = subtitle {
            ui.label(RichText::new(s).size(12.0).color(TEXT2));
        }
        ui.add_space(8.0);
        add(ui)
    })
}

/// Page title with a muted subtitle.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(24.0).strong().color(TEXT));
    ui.label(RichText::new(subtitle).size(13.0).color(TEXT2));
    ui.add_space(14.0);
}

/// Small uppercase muted label used to group controls.
#[allow(dead_code)] // Reserved for pages that group several controls.
pub fn section_header(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text.to_uppercase()).size(11.0).strong().color(TEXT2));
}

/// Primary (blue) button.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
            .fill(BLUE)
            .stroke(Stroke::new(1.0, BLUE_LIGHT))
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// Neutral surface button.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(TEXT))
            .fill(SURFACE2)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// Destructive button (red text, neutral surface).
pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(theme::DANGER))
            .fill(SURFACE2)
            .stroke(Stroke::new(1.0, theme::DANGER.gamma_multiply(0.6)))
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// Animated on/off switch. Sets `on` and returns a response whose `changed()` is true on click.
pub fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let bg = lerp_color(SURFACE2, BLUE, t);
        p.rect_filled(rect, rect.height() / 2.0, bg);
        p.rect_stroke(
            rect,
            rect.height() / 2.0,
            Stroke::new(1.0, lerp_color(BORDER, BLUE_LIGHT, t)),
            StrokeKind::Inside,
        );
        let r = rect.height() / 2.0 - 3.0;
        let x = egui::lerp(
            (rect.left() + rect.height() / 2.0)..=(rect.right() - rect.height() / 2.0),
            t,
        );
        p.circle_filled(Pos2::new(x, rect.center().y), r, Color32::WHITE);
    }
    resp
}

/// Label + optional description on the left, switch on the right. Returns true when toggled.
pub fn toggle_row(ui: &mut Ui, label: &str, description: Option<&str>, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(14.0).color(TEXT));
            if let Some(d) = description {
                ui.label(RichText::new(d).size(12.0).color(TEXT2));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = toggle_switch(ui, on).changed();
        });
    });
    changed
}

/// Segmented control. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    egui::Frame::new()
        .fill(SURFACE2)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(theme::RADIUS_SM)
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (value, label) in options {
                    let active = *current == *value;
                    let text = RichText::new(*label).color(if active { Color32::WHITE } else { TEXT2 });
                    let btn = egui::Button::new(text)
                        .fill(if active { BLUE } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(theme::RADIUS_SM);
                    if ui.add(btn).clicked() && !active {
                        *current = *value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Monospace key cap, used for hotkey strings.
pub fn key_chip(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(SURFACE2)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(4.0)
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::new(12.0, FontFamily::Monospace))
                    .color(TEXT),
            );
        });
}

/// Rounded colored label, e.g. "Active" / "Off".
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .corner_radius(99.0)
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).strong().color(color));
        });
}

/// Vector icons, drawn on a 24x24 grid (stroke style, scaled to the requested size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Not every icon is used by the sidebar; others may use the rest.
pub enum Icon {
    Overview,
    Editor,
    Presets,
    Profiles,
    Settings,
    Power,
    Eye,
    Plus,
}

/// Paint an icon into a `size` x `size` box (allocated in the layout).
pub fn icon(ui: &mut Ui, kind: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_icon(ui.painter(), rect, kind, color);
    }
    resp
}

/// Paint an icon directly with a painter inside `rect` (used by clickable nav items).
pub fn paint_icon(p: &egui::Painter, rect: Rect, kind: Icon, color: Color32) {
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
    }
}

/// Linear interpolation between two opaque colors (t in 0..=1).
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp_color(SURFACE2, BLUE, 0.0), SURFACE2);
        assert_eq!(lerp_color(SURFACE2, BLUE, 1.0), BLUE);
        assert_eq!(lerp_color(SURFACE2, BLUE, 2.0), BLUE);
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
}
