//! System tray icon and menu.
//!
//! Windows: real implementation on top of `tray-icon` (muda menus). Events are drained with
//! [`Tray::poll`] from the UI thread. Non-Windows: a no-op stub with the same API so the crate
//! builds and tests run on Linux.

/// Actions the user triggered from the tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// "Show OpenCrosshair" menu item, or left click / double click on the tray icon.
    Show,
    ToggleCrosshair,
    ToggleOverlay,
    Quit,
}

const SIZE: u32 = 32;
const BG: [f32; 3] = [16.0, 19.0, 24.0]; // #101318
const BLUE: [f32; 3] = [59.0, 130.0, 246.0]; // #3B82F6
const CYAN: [f32; 3] = [34.0, 211.0, 238.0]; // #22D3EE

/// Procedural 32x32 tray icon (same design as `assets/icons/make_icons.py`).
/// Returns `(rgba, width, height)` with non-premultiplied RGBA bytes.
pub fn icon_rgba() -> (Vec<u8>, u32, u32) {
    let s = SIZE as f32;
    let half = s / 2.0;
    let radius = 0.22 * s;
    let arm_outer = 0.40 * s;
    let arm_inner = 0.12 * s;
    let thick = (0.06 * s).max(1.0);
    let ring_r = 0.30 * s;
    let ring_w = (0.045 * s).max(0.9);
    let dot_r = 0.075 * s;

    let mut out = vec![0u8; (SIZE * SIZE * 4) as usize];
    for j in 0..SIZE {
        for i in 0..SIZE {
            // Pixel centre relative to the icon centre.
            let px = i as f32 + 0.5 - half;
            let py = j as f32 + 0.5 - half;

            let a_bg = coverage(rounded_box_sd(px, py, half - 0.5, radius));
            if a_bg <= 0.0 {
                continue;
            }
            let arms = [
                (-arm_outer, 0.0, -arm_inner, 0.0),
                (arm_inner, 0.0, arm_outer, 0.0),
                (0.0, -arm_outer, 0.0, -arm_inner),
                (0.0, arm_inner, 0.0, arm_outer),
            ];
            let d_arms = arms
                .iter()
                .map(|&(ax, ay, bx, by)| seg_dist(px, py, ax, ay, bx, by) - thick / 2.0)
                .fold(f32::INFINITY, f32::min);
            let r = (px * px + py * py).sqrt();
            let d_ring = (r - ring_r).abs() - ring_w / 2.0;
            let a_blue = coverage(d_arms.min(d_ring));
            let a_dot = coverage(r - dot_r);

            let mut c = BG;
            for k in 0..3 {
                c[k] += (BLUE[k] - c[k]) * a_blue;
                c[k] += (CYAN[k] - c[k]) * a_dot;
            }
            let o = ((j * SIZE + i) * 4) as usize;
            out[o] = c[0].round() as u8;
            out[o + 1] = c[1].round() as u8;
            out[o + 2] = c[2].round() as u8;
            out[o + 3] = (255.0 * a_bg).round() as u8;
        }
    }
    (out, SIZE, SIZE)
}

fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (vx, vy) = (bx - ax, by - ay);
    let (wx, wy) = (px - ax, py - ay);
    let t = ((wx * vx + wy * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    let (dx, dy) = (px - (ax + t * vx), py - (ay + t * vy));
    (dx * dx + dy * dy).sqrt()
}

fn rounded_box_sd(px: f32, py: f32, half: f32, radius: f32) -> f32 {
    let qx = px.abs() - half + radius;
    let qy = py.abs() - half + radius;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - radius
}

fn coverage(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

#[cfg(windows)]
mod imp {
    use super::{TrayEvent, icon_rgba};
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct Tray {
        // Kept alive for the lifetime of the tray; dropping it removes the icon.
        _icon: TrayIcon,
        show_id: MenuId,
        crosshair_id: MenuId,
        overlay_id: MenuId,
        quit_id: MenuId,
        crosshair_item: CheckMenuItem,
        overlay_item: CheckMenuItem,
    }

    fn err<E: std::fmt::Display>(e: E) -> String {
        e.to_string()
    }

    impl Tray {
        /// Create the tray icon and its context menu. Must be called on the UI thread.
        pub fn new() -> Result<Tray, String> {
            let show = MenuItem::new("Show OpenCrosshair", true, None);
            let crosshair_item = CheckMenuItem::new("Toggle crosshair", true, true, None);
            let overlay_item = CheckMenuItem::new("Toggle overlay", true, true, None);
            let quit = MenuItem::new("Quit", true, None);

            let menu = Menu::new();
            menu.append(&show).map_err(err)?;
            menu.append(&PredefinedMenuItem::separator()).map_err(err)?;
            menu.append(&crosshair_item).map_err(err)?;
            menu.append(&overlay_item).map_err(err)?;
            menu.append(&PredefinedMenuItem::separator()).map_err(err)?;
            menu.append(&quit).map_err(err)?;

            let (rgba, w, h) = icon_rgba();
            let icon = Icon::from_rgba(rgba, w, h).map_err(err)?;
            let icon = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_tooltip("OpenCrosshair")
                .with_icon(icon)
                .build()
                .map_err(err)?;

            Ok(Tray {
                _icon: icon,
                show_id: show.id().clone(),
                crosshair_id: crosshair_item.id().clone(),
                overlay_id: overlay_item.id().clone(),
                quit_id: quit.id().clone(),
                crosshair_item,
                overlay_item,
            })
        }

        /// Drain pending menu and tray-icon events without blocking.
        pub fn poll(&mut self) -> Vec<TrayEvent> {
            let mut out = Vec::new();
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                let mapped = if ev.id == self.show_id {
                    TrayEvent::Show
                } else if ev.id == self.crosshair_id {
                    TrayEvent::ToggleCrosshair
                } else if ev.id == self.overlay_id {
                    TrayEvent::ToggleOverlay
                } else if ev.id == self.quit_id {
                    TrayEvent::Quit
                } else {
                    continue;
                };
                out.push(mapped);
            }
            while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                let show = matches!(
                    ev,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );
                // A double click also produces a single click; report Show once per poll.
                if show && !out.contains(&TrayEvent::Show) {
                    out.push(TrayEvent::Show);
                }
            }
            out
        }

        /// Reflect the current crosshair / overlay state in the menu check marks.
        pub fn set_states(&mut self, crosshair_visible: bool, overlay_enabled: bool) {
            self.crosshair_item.set_checked(crosshair_visible);
            self.overlay_item.set_checked(overlay_enabled);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::TrayEvent;

    /// Non-Windows stub: no tray icon, never produces events.
    pub struct Tray;

    impl Tray {
        pub fn new() -> Result<Tray, String> {
            Ok(Tray)
        }

        pub fn poll(&mut self) -> Vec<TrayEvent> {
            Vec::new()
        }

        pub fn set_states(&mut self, _crosshair_visible: bool, _overlay_enabled: bool) {}
    }
}

pub use imp::Tray;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_rgba_has_expected_dimensions_and_length() {
        let (rgba, w, h) = icon_rgba();
        assert_eq!((w, h), (32, 32));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }

    #[test]
    fn icon_rgba_corners_transparent_centre_is_cyan_dot() {
        let (rgba, w, _) = icon_rgba();
        let px = |x: u32, y: u32| {
            let o = ((y * w + x) * 4) as usize;
            [rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3]]
        };
        assert_eq!(px(0, 0)[3], 0, "corner must be transparent");
        let centre = px(16, 16);
        assert_eq!(centre[3], 255);
        // Centre dot is cyan: green and blue dominate red.
        assert!(centre[1] > 150 && centre[2] > 150 && centre[0] < 100, "{centre:?}");
        // Background away from the crosshair is the dark app background.
        let bg = px(6, 6);
        assert_eq!(bg[3], 255);
        assert!(bg[0] < 40 && bg[1] < 40 && bg[2] < 50);
    }
}
