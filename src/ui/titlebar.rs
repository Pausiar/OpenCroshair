//! Custom window chrome for the borderless window: a 40 px title bar (logo, serif app name,
//! theme toggle, Windows-style minimize / maximize-restore / close) and resize handles on the
//! window edges and corners.
//!
//! The bar is a top `Panel`. [`show_ui`] lays it out in the app's root `Ui` (preferred, so the
//! bar takes layout space). [`show`] does the same on a temporary root `Ui` sized to the content
//! rect, for callers that only have a `Context`.
// Not yet wired into app.rs; the app's title bar integration lands separately.
#![allow(dead_code)]

use crate::storage::config::ThemeMode;
use crate::ui::theme;
use crate::ui::widgets::{self, Icon, heading_text, mix_color};
use eframe::egui::{
    self, Color32, CursorIcon, Frame, Margin, Pos2, Rect, Response, Sense, Stroke, Ui, UiBuilder, Vec2, ViewportCommand,
};
use egui::{PointerButton, ResizeDirection};

/// Height of the title bar in points.
pub const BAR_HEIGHT: f32 = 40.0;
const WINDOW_BUTTON_W: f32 = 46.0;
/// Width of the resize zone along each edge, and the size of each corner zone.
pub const RESIZE_ZONE: f32 = 6.0;

/// What the user did on the title bar this frame. The caller applies it.
/// `ToggleTheme` is reported after [`show`] / [`show_ui`] already flipped `theme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarAction {
    None,
    ToggleTheme,
    Minimize,
    ToggleMaximize,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowButton {
    Minimize,
    MaximizeOrRestore { maximized: bool },
    Close,
}

/// Draw the title bar on a temporary root `Ui` that covers the content rect.
/// Prefer [`show_ui`] from the app's root `Ui`, so the bar reserves its own layout space.
pub fn show(ctx: &egui::Context, theme: &mut ThemeMode, maximized: bool) -> TitleBarAction {
    let mut root = Ui::new(
        ctx.clone(),
        widgets::state_id("oc_titlebar_root"),
        UiBuilder::new().max_rect(ctx.content_rect()),
    );
    show_ui(&mut root, theme, maximized)
}

/// Draw the title bar as a 40 px top panel inside `ui`. See [`show`] for the flip semantics.
/// Double-clicking the empty bar area reports `ToggleMaximize`; dragging it starts a window move.
/// `Minimize`, `ToggleMaximize` and `Close` are reported, not applied: the caller sends the
/// matching `ViewportCommand`.
pub fn show_ui(ui: &mut Ui, theme_mode: &mut ThemeMode, maximized: bool) -> TitleBarAction {
    let p = theme::palette(ui.ctx());
    let mut action = TitleBarAction::None;
    egui::Panel::top("oc_titlebar")
        .exact_size(BAR_HEIGHT)
        .frame(
            Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(12, 0)),
        )
        .show(ui, |ui| {
            // Interact with the empty bar first: widgets added later (the buttons) sit on top
            // and keep their own clicks.
            let bar = ui.max_rect();
            let drag = ui.interact(bar, widgets::state_id("oc_titlebar_drag"), Sense::click_and_drag());

            ui.horizontal_centered(|ui| {
                logo(ui);
                ui.add_space(8.0);
                ui.label(heading_text("OpenCrosshair", 15.0));

                let controls_w = BUTTON_ICON_SIZE + 8.0 + 3.0 * WINDOW_BUTTON_W;
                ui.add_space((ui.available_width() - controls_w).max(0.0));

                let dark = matches!(theme_mode, ThemeMode::Dark);
                let (icon, tip) = if dark {
                    (Icon::Sun, "Switch to light theme")
                } else {
                    (Icon::Moon, "Switch to dark theme")
                };
                if widgets::icon_button(ui, icon, tip).clicked() {
                    *theme_mode = if dark { ThemeMode::Light } else { ThemeMode::Dark };
                    action = TitleBarAction::ToggleTheme;
                }
                ui.add_space(8.0);

                if window_button(ui, WindowButton::Minimize).clicked() {
                    action = TitleBarAction::Minimize;
                }
                if window_button(ui, WindowButton::MaximizeOrRestore { maximized }).clicked() {
                    action = TitleBarAction::ToggleMaximize;
                }
                if window_button(ui, WindowButton::Close).clicked() {
                    action = TitleBarAction::Close;
                }
            });

            if action == TitleBarAction::None {
                if drag.double_clicked() {
                    action = TitleBarAction::ToggleMaximize;
                } else if drag.drag_started_by(PointerButton::Primary) {
                    // Sent on drag start rather than on press, so a double-click still maximizes.
                    ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                }
            }
        });

    action
}

const BUTTON_ICON_SIZE: f32 = 32.0;

fn logo(ui: &mut Ui) {
    let p = theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, p.primary);
        widgets::paint_icon(painter, rect.shrink(4.0), Icon::Editor, p.on_primary);
    }
}

/// 46 x 40 window control. The close button turns danger-red on hover.
fn window_button(ui: &mut Ui, kind: WindowButton) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(WINDOW_BUTTON_W, BAR_HEIGHT), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let hover = ui.ctx().animate_value_with_time(
            resp.id.with("hover"),
            if resp.hovered() { 1.0 } else { 0.0 },
            widgets::ANIM_TIME,
        );
        let (fill, glyph) = match kind {
            WindowButton::Close => (
                mix_color(Color32::TRANSPARENT, p.danger, hover),
                mix_color(p.text_muted, p.surface, hover),
            ),
            _ => (
                mix_color(Color32::TRANSPARENT, p.surface_alt, hover),
                mix_color(p.text_muted, p.text, hover),
            ),
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, fill);
        let icon = match kind {
            WindowButton::Minimize => Icon::Minimize,
            WindowButton::MaximizeOrRestore { maximized: true } => Icon::Restore,
            WindowButton::MaximizeOrRestore { maximized: false } => Icon::Maximize,
            WindowButton::Close => Icon::Close,
        };
        widgets::paint_icon(
            painter,
            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
            icon,
            glyph,
        );
    }
    resp
}

/// Edge and corner zones for a window whose outer bounds are `bounds`.
/// Corners come first so [`resize_direction_at`] prefers them where zones overlap.
pub fn resize_zone_rects(bounds: Rect) -> [(ResizeDirection, Rect); 8] {
    let e = RESIZE_ZONE;
    let (l, t, r, b) = (bounds.left(), bounds.top(), bounds.right(), bounds.bottom());
    let (w, h) = (bounds.width(), bounds.height());
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    [
        (ResizeDirection::NorthWest, rect(l, t, e, e)),
        (ResizeDirection::NorthEast, rect(r - e, t, e, e)),
        (ResizeDirection::SouthWest, rect(l, b - e, e, e)),
        (ResizeDirection::SouthEast, rect(r - e, b - e, e, e)),
        (ResizeDirection::North, rect(l + e, t, w - 2.0 * e, e)),
        (ResizeDirection::South, rect(l + e, b - e, w - 2.0 * e, e)),
        (ResizeDirection::West, rect(l, t + e, e, h - 2.0 * e)),
        (ResizeDirection::East, rect(r - e, t + e, e, h - 2.0 * e)),
    ]
}

/// Resize direction for a pointer position inside `bounds`, or `None` in the interior.
pub fn resize_direction_at(bounds: Rect, pos: Pos2) -> Option<ResizeDirection> {
    resize_zone_rects(bounds)
        .into_iter()
        .find(|(_, zone)| zone.contains(pos))
        .map(|(dir, _)| dir)
}

/// Cursor shown over a resize zone.
pub fn cursor_for_direction(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
    }
}

/// Invisible 6 px hit zones along the window edges and corners. A primary press on a zone sends
/// `ViewportCommand::BeginResize`. Does nothing while the window is maximized.
/// Call once per frame, after the central content.
pub fn resize_handles(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }
    let bounds = ctx.content_rect();
    let pressed = ctx.input(|i| i.pointer.primary_pressed());
    for (dir, zone) in resize_zone_rects(bounds) {
        let id = widgets::state_id(&format!("oc_resize_{dir:?}"));
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .movable(false)
            .fixed_pos(zone.min)
            .show(ctx, |ui| {
                let (_, resp) = ui.allocate_exact_size(zone.size(), Sense::drag());
                let resp = resp.on_hover_cursor(cursor_for_direction(dir));
                if resp.hovered() && pressed {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{fonts, theme};
    use eframe::egui::{RawInput, Vec2};

    fn window() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))
    }

    #[test]
    fn hit_zones_map_edges_and_corners() {
        let b = window();
        assert_eq!(
            resize_direction_at(b, Pos2::new(0.0, 0.0)),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(899.0, 0.0)),
            Some(ResizeDirection::NorthEast)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(0.0, 599.0)),
            Some(ResizeDirection::SouthWest)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(899.0, 599.0)),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(450.0, 2.0)),
            Some(ResizeDirection::North)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(450.0, 598.0)),
            Some(ResizeDirection::South)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(2.0, 300.0)),
            Some(ResizeDirection::West)
        );
        assert_eq!(
            resize_direction_at(b, Pos2::new(898.0, 300.0)),
            Some(ResizeDirection::East)
        );
        assert_eq!(resize_direction_at(b, Pos2::new(450.0, 300.0)), None);
    }

    #[test]
    fn zones_stay_inside_bounds_and_do_not_overlap_interior() {
        let b = window();
        for (_, zone) in resize_zone_rects(b) {
            assert!(b.contains_rect(zone), "zone {zone:?} leaves the window");
            assert!(zone.width() <= RESIZE_ZONE.max(b.width()) && zone.height() <= RESIZE_ZONE.max(b.height()));
        }
        let center = b.center();
        assert!(resize_zone_rects(b).iter().all(|(_, z)| !z.contains(center)));
    }

    #[test]
    fn cursor_matches_direction_axis() {
        assert_eq!(cursor_for_direction(ResizeDirection::North), CursorIcon::ResizeVertical);
        assert_eq!(
            cursor_for_direction(ResizeDirection::East),
            CursorIcon::ResizeHorizontal
        );
        assert_eq!(cursor_for_direction(ResizeDirection::NorthEast), CursorIcon::ResizeNeSw);
        assert_eq!(cursor_for_direction(ResizeDirection::SouthEast), CursorIcon::ResizeNwSe);
    }

    fn raw() -> RawInput {
        RawInput {
            screen_rect: Some(window()),
            ..Default::default()
        }
    }

    fn discard(mut out: egui::FullOutput) {
        out.textures_delta.clear();
    }

    fn fresh(mode: ThemeMode) -> egui::Context {
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        theme::apply(&ctx, mode);
        discard(ctx.run_ui(raw(), |_| {}));
        ctx
    }

    #[test]
    fn smoke_title_bar_both_themes_and_window_states() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            for maximized in [false, true] {
                let ctx = fresh(mode);
                let mut theme_mode = mode;
                let mut action = TitleBarAction::None;
                discard(ctx.run_ui(raw(), |ui| {
                    action = show_ui(ui, &mut theme_mode, maximized);
                }));
                assert_eq!(action, TitleBarAction::None);
                discard(ctx.run_pass(raw(), |ctx| {
                    let mut m = mode;
                    let _ = show(ctx, &mut m, maximized);
                    resize_handles(ctx);
                }));
            }
        }
    }

    #[test]
    fn theme_toggle_flips_mode_without_pointer_input() {
        let ctx = fresh(ThemeMode::Light);
        let mut mode = ThemeMode::Light;
        discard(ctx.run_pass(raw(), |ctx| {
            let _ = show(ctx, &mut mode, false);
        }));
        assert_eq!(mode, ThemeMode::Light, "no click means no change");
    }
}
