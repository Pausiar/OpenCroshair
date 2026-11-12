//! Pure positioning math (compiles and is unit-tested on every OS), plus the Windows-only
//! foreground window query used by `CenterMode::GameWindow`.
//!
//! All coordinates are physical pixels on the virtual desktop. Crosshair offsets are authored
//! at 1x and are multiplied by the DPI scale of the monitor the anchor lies on; the
//! `PositionSettings` offsets are already physical and are never scaled.
use crate::crosshair::model::Crosshair;
use crate::storage::config::{CenterMode, PositionSettings};
use crate::system::monitors::MonitorInfo;

/// Axis-aligned rectangle in physical pixels (`x`,`y` = top-left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn is_valid(&self) -> bool {
        self.w > 0 && self.h > 0
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x.saturating_add(self.w / 2), self.y.saturating_add(self.h / 2))
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && py >= self.y
            && (px as i64) < self.x as i64 + self.w as i64
            && (py as i64) < self.y as i64 + self.h as i64
    }
}

/// Used when no monitor could be enumerated.
fn fallback_monitor() -> MonitorInfo {
    MonitorInfo {
        index: 0,
        name: "Display 1 (1920x1080)".into(),
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        primary: true,
        scale: 1.0,
    }
}

fn primary_or_first(monitors: &[MonitorInfo]) -> MonitorInfo {
    monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| monitors.first())
        .cloned()
        .unwrap_or_else(fallback_monitor)
}

/// Monitor selected by `PositionSettings::monitor`; unknown or `None` falls back to primary.
fn selected_monitor(monitors: &[MonitorInfo], idx: Option<usize>) -> MonitorInfo {
    idx.and_then(|i| monitors.iter().find(|m| m.index == i))
        .cloned()
        .unwrap_or_else(|| primary_or_first(monitors))
}

fn monitor_rect(m: &MonitorInfo) -> Rect {
    Rect {
        x: m.x,
        y: m.y,
        w: m.width,
        h: m.height,
    }
}

/// Anchor point (crosshair centre) before crosshair/position offsets are applied.
fn anchor_point(monitors: &[MonitorInfo], game_rect: Option<Rect>, pos: &PositionSettings) -> (i32, i32) {
    match pos.mode {
        CenterMode::Custom => (pos.custom_x, pos.custom_y),
        CenterMode::GameWindow => match game_rect.filter(Rect::is_valid) {
            Some(r) => r.center(),
            None => monitor_rect(&selected_monitor(monitors, pos.monitor)).center(),
        },
        CenterMode::Monitor => monitor_rect(&selected_monitor(monitors, pos.monitor)).center(),
    }
}

/// DPI scale of the monitor that contains the anchor point. Falls back to the selected
/// monitor (then 1.0) when the anchor lies outside every monitor, e.g. a custom position
/// off-screen. The renderer uses this scale so the bitmap matches the target monitor.
pub fn dpi_scale_for(monitors: &[MonitorInfo], game_rect: Option<Rect>, pos: &PositionSettings) -> f32 {
    let (ax, ay) = anchor_point(monitors, game_rect, pos);
    let containing = monitors.iter().find(|m| monitor_rect(m).contains(ax, ay));
    let chosen = containing
        .cloned()
        .unwrap_or_else(|| selected_monitor(monitors, pos.monitor));
    if chosen.scale.is_finite() && chosen.scale > 0.0 {
        chosen.scale
    } else {
        1.0
    }
}

/// Top-left window position (physical px) for a bitmap of `bitmap_w` x `bitmap_h`, at 1x
/// crosshair offsets (scale 1.0). See [`compute_position_scaled`].
#[allow(dead_code)] // Contract entry point; the overlay thread uses the scaled variant.
pub fn compute_position(
    monitors: &[MonitorInfo],
    game_rect: Option<Rect>,
    pos: &PositionSettings,
    crosshair: &Crosshair,
    bitmap_w: u32,
    bitmap_h: u32,
) -> (i32, i32) {
    compute_position_scaled(monitors, game_rect, pos, crosshair, bitmap_w, bitmap_h, 1.0)
}

/// Top-left window position (physical px). The bitmap centre is placed on
/// `anchor + pos.offset + crosshair.offset * scale`.
///
/// - `Monitor`: centre of `pos.monitor` (by `MonitorInfo::index`), primary if missing.
/// - `GameWindow`: centre of `game_rect` when valid, otherwise the `Monitor` behaviour.
/// - `Custom`: `pos.custom_x` / `pos.custom_y` as absolute virtual-desktop coordinates.
pub fn compute_position_scaled(
    monitors: &[MonitorInfo],
    game_rect: Option<Rect>,
    pos: &PositionSettings,
    crosshair: &Crosshair,
    bitmap_w: u32,
    bitmap_h: u32,
    scale: f32,
) -> (i32, i32) {
    let s = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let (ax, ay) = anchor_point(monitors, game_rect, pos);
    let cx = ax
        .saturating_add(pos.offset_x)
        .saturating_add((crosshair.offset_x * s).round() as i32);
    let cy = ay
        .saturating_add(pos.offset_y)
        .saturating_add((crosshair.offset_y * s).round() as i32);
    // Bitmaps are centred with the centre pixel at index w/2 (integer division).
    let half_w = (bitmap_w / 2) as i32;
    let half_h = (bitmap_h / 2) as i32;
    (cx.saturating_sub(half_w), cy.saturating_sub(half_h))
}

/// Rectangle of the foreground window, or `None` when there is no usable window.
///
/// Ignored: this overlay's own window, windows of this process (so the settings UI never
/// becomes the "game"), minimized windows, invisible windows and zero-size rectangles.
#[cfg(windows)]
pub fn foreground_window_rect() -> Option<Rect> {
    use crate::overlay::windows_api;
    let hwnd = windows_api::foreground_hwnd()?;
    if windows_api::is_overlay_window(hwnd) {
        return None;
    }
    if windows_api::is_iconic(hwnd) || !windows_api::is_visible(hwnd) {
        return None;
    }
    if windows_api::window_pid(hwnd) == windows_api::current_pid() {
        return None;
    }
    let (l, t, r, b) = windows_api::window_rect(hwnd)?;
    let rect = Rect {
        x: l,
        y: t,
        w: r.saturating_sub(l),
        h: b.saturating_sub(t),
    };
    rect.is_valid().then_some(rect)
}

/// Non-Windows stub: there is no foreground game window to follow.
#[cfg(not(windows))]
pub fn foreground_window_rect() -> Option<Rect> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(index: usize, x: i32, y: i32, w: i32, h: i32, primary: bool, scale: f32) -> MonitorInfo {
        MonitorInfo {
            index,
            name: format!("Display {}", index + 1),
            x,
            y,
            width: w,
            height: h,
            primary,
            scale,
        }
    }

    fn single() -> Vec<MonitorInfo> {
        vec![mon(0, 0, 0, 1920, 1080, true, 1.0)]
    }

    fn pos(mode: CenterMode) -> PositionSettings {
        PositionSettings {
            mode,
            ..PositionSettings::default()
        }
    }

    fn ch() -> Crosshair {
        Crosshair::default()
    }

    #[test]
    fn primary_monitor_centre_with_odd_bitmap() {
        // 101x101 bitmap: centre pixel index 50 lands on (960, 540).
        let p = compute_position(&single(), None, &pos(CenterMode::Monitor), &ch(), 101, 101);
        assert_eq!(p, (910, 490));
    }

    #[test]
    fn even_bitmap_uses_half_size_floor() {
        let p = compute_position(&single(), None, &pos(CenterMode::Monitor), &ch(), 100, 100);
        assert_eq!(p, (910, 490));
        let p = compute_position(&single(), None, &pos(CenterMode::Monitor), &ch(), 1, 1);
        assert_eq!(p, (960, 540));
    }

    #[test]
    fn selects_monitor_by_index_not_by_position_in_slice() {
        // Slice order differs from index order on purpose.
        let mons = vec![
            mon(2, -2560, 0, 2560, 1440, false, 1.0),
            mon(0, 0, 0, 1920, 1080, true, 1.0),
            mon(1, 1920, 0, 1920, 1080, false, 1.0),
        ];
        let mut p = pos(CenterMode::Monitor);
        p.monitor = Some(2);
        // centre of left monitor: (-2560 + 1280, 720) = (-1280, 720)
        assert_eq!(compute_position(&mons, None, &p, &ch(), 101, 101), (-1330, 670));
        p.monitor = Some(1);
        // centre of right monitor: (1920 + 960, 540) = (2880, 540)
        assert_eq!(compute_position(&mons, None, &p, &ch(), 101, 101), (2830, 490));
    }

    #[test]
    fn none_monitor_uses_primary_even_if_listed_later() {
        let mons = vec![
            mon(0, -1920, -200, 1920, 1080, false, 1.0),
            mon(1, 0, 0, 1920, 1080, true, 1.0),
        ];
        let p = pos(CenterMode::Monitor);
        assert_eq!(compute_position(&mons, None, &p, &ch(), 101, 101), (910, 490));
    }

    #[test]
    fn unknown_monitor_index_falls_back_to_primary() {
        let mut p = pos(CenterMode::Monitor);
        p.monitor = Some(42);
        assert_eq!(compute_position(&single(), None, &p, &ch(), 101, 101), (910, 490));
    }

    #[test]
    fn negative_coordinate_monitor_left_of_and_above_primary() {
        let mons = vec![
            mon(0, 0, 0, 1920, 1080, true, 1.0),
            mon(1, -1920, -200, 1920, 1080, false, 1.0),
        ];
        let mut p = pos(CenterMode::Monitor);
        p.monitor = Some(1);
        // centre (-960, 340)
        assert_eq!(compute_position(&mons, None, &p, &ch(), 101, 101), (-1010, 290));
    }

    #[test]
    fn position_offsets_applied_unscaled() {
        let mut p = pos(CenterMode::Monitor);
        p.offset_x = 10;
        p.offset_y = -5;
        assert_eq!(
            compute_position_scaled(&single(), None, &p, &ch(), 101, 101, 2.0),
            (920, 485)
        );
    }

    #[test]
    fn crosshair_offsets_applied_at_1x() {
        let mut c = ch();
        c.offset_x = 3.0;
        c.offset_y = -4.0;
        assert_eq!(
            compute_position(&single(), None, &pos(CenterMode::Monitor), &c, 101, 101),
            (913, 486)
        );
    }

    #[test]
    fn crosshair_offsets_scaled_by_dpi_and_rounded() {
        let mut c = ch();
        c.offset_x = 3.0;
        c.offset_y = -4.0;
        // scale 1.5: 4.5 rounds to 5 (half away from zero), -6.0 stays -6
        assert_eq!(
            compute_position_scaled(&single(), None, &pos(CenterMode::Monitor), &c, 151, 151, 1.5),
            (960 + 5 - 75, 540 - 6 - 75)
        );
        c.offset_x = 1.4;
        c.offset_y = 0.0;
        assert_eq!(
            compute_position_scaled(&single(), None, &pos(CenterMode::Monitor), &c, 101, 101, 1.0),
            (961 - 50, 540 - 50)
        );
    }

    #[test]
    fn custom_mode_uses_absolute_point_and_ignores_monitor() {
        let mut p = pos(CenterMode::Custom);
        p.custom_x = -500;
        p.custom_y = 300;
        p.monitor = Some(1); // ignored in Custom mode
        assert_eq!(compute_position(&single(), None, &p, &ch(), 101, 101), (-550, 250));
    }

    #[test]
    fn custom_mode_with_offsets() {
        let mut p = pos(CenterMode::Custom);
        p.custom_x = 100;
        p.custom_y = 100;
        p.offset_x = -20;
        p.offset_y = 30;
        let mut c = ch();
        c.offset_x = 5.0;
        assert_eq!(
            compute_position(&single(), None, &p, &c, 11, 11),
            (100 - 20 + 5 - 5, 100 + 30 - 5)
        );
    }

    #[test]
    fn game_window_centres_on_valid_rect() {
        let r = Rect {
            x: 100,
            y: 200,
            w: 800,
            h: 600,
        };
        let p = pos(CenterMode::GameWindow);
        // centre (500, 500)
        assert_eq!(compute_position(&single(), Some(r), &p, &ch(), 101, 101), (450, 450));
    }

    #[test]
    fn game_window_on_negative_monitor_rect() {
        let r = Rect {
            x: -2400,
            y: -300,
            w: 1280,
            h: 720,
        };
        let p = pos(CenterMode::GameWindow);
        // centre (-1760, 60)
        assert_eq!(compute_position(&single(), Some(r), &p, &ch(), 101, 101), (-1810, 10));
    }

    #[test]
    fn game_window_without_rect_falls_back_to_monitor() {
        let p = pos(CenterMode::GameWindow);
        assert_eq!(compute_position(&single(), None, &p, &ch(), 101, 101), (910, 490));
    }

    #[test]
    fn game_window_zero_size_rect_falls_back_to_monitor() {
        let p = pos(CenterMode::GameWindow);
        let zero_w = Rect {
            x: 100,
            y: 100,
            w: 0,
            h: 600,
        };
        let neg_h = Rect {
            x: 100,
            y: 100,
            w: 600,
            h: -1,
        };
        assert_eq!(
            compute_position(&single(), Some(zero_w), &p, &ch(), 101, 101),
            (910, 490)
        );
        assert_eq!(
            compute_position(&single(), Some(neg_h), &p, &ch(), 101, 101),
            (910, 490)
        );
    }

    #[test]
    fn game_window_fallback_uses_selected_monitor() {
        let mons = vec![
            mon(0, 0, 0, 1920, 1080, true, 1.0),
            mon(1, 1920, 0, 2560, 1440, false, 1.0),
        ];
        let mut p = pos(CenterMode::GameWindow);
        p.monitor = Some(1);
        // centre (1920 + 1280, 720)
        assert_eq!(
            compute_position(&mons, None, &p, &ch(), 101, 101),
            (3200 - 50, 720 - 50)
        );
    }

    #[test]
    fn empty_monitor_list_uses_1920x1080_fallback() {
        let p = pos(CenterMode::Monitor);
        assert_eq!(compute_position(&[], None, &p, &ch(), 101, 101), (910, 490));
        assert_eq!(dpi_scale_for(&[], None, &p), 1.0);
    }

    #[test]
    fn extreme_values_do_not_overflow() {
        let mut p = pos(CenterMode::Custom);
        p.custom_x = i32::MAX;
        p.custom_y = i32::MIN;
        p.offset_x = i32::MAX;
        p.offset_y = i32::MIN;
        let mut c = ch();
        c.offset_x = 1.0e9;
        c.offset_y = -1.0e9;
        let (x, y) = compute_position(&single(), None, &p, &c, u32::MAX, u32::MAX);
        assert!(x <= i32::MAX && y >= i32::MIN);
    }

    #[test]
    fn dpi_scale_follows_anchor_monitor() {
        let mons = vec![
            mon(0, 0, 0, 1920, 1080, true, 1.0),
            mon(1, 1920, 0, 2560, 1440, false, 2.0),
        ];
        let mut p = pos(CenterMode::Monitor);
        p.monitor = Some(1);
        assert_eq!(dpi_scale_for(&mons, None, &p), 2.0);
        p.monitor = None;
        assert_eq!(dpi_scale_for(&mons, None, &p), 1.0);
    }

    #[test]
    fn dpi_scale_uses_game_rect_monitor() {
        let mons = vec![
            mon(0, 0, 0, 1920, 1080, true, 1.0),
            mon(1, 1920, 0, 2560, 1440, false, 1.5),
        ];
        let p = pos(CenterMode::GameWindow);
        let on_second = Rect {
            x: 2000,
            y: 100,
            w: 800,
            h: 600,
        };
        assert_eq!(dpi_scale_for(&mons, Some(on_second), &p), 1.5);
        assert_eq!(dpi_scale_for(&mons, None, &p), 1.0);
    }

    #[test]
    fn dpi_scale_for_offscreen_custom_point_uses_selected_monitor() {
        let mons = vec![mon(0, 0, 0, 1920, 1080, true, 1.25)];
        let mut p = pos(CenterMode::Custom);
        p.custom_x = 50_000;
        p.custom_y = 50_000;
        assert_eq!(dpi_scale_for(&mons, None, &p), 1.25);
    }

    #[test]
    fn dpi_scale_rejects_invalid_values() {
        let mons = vec![mon(0, 0, 0, 1920, 1080, true, 0.0)];
        assert_eq!(dpi_scale_for(&mons, None, &pos(CenterMode::Monitor)), 1.0);
    }

    #[test]
    fn rect_helpers() {
        let r = Rect {
            x: -10,
            y: -20,
            w: 30,
            h: 40,
        };
        assert!(r.is_valid());
        assert_eq!(r.center(), (5, 0));
        assert!(r.contains(-10, -20));
        assert!(!r.contains(20, 0));
        assert!(!r.contains(0, 20));
        assert!(!Rect { x: 0, y: 0, w: 0, h: 5 }.is_valid());
    }

    #[cfg(not(windows))]
    #[test]
    fn foreground_rect_is_none_off_windows() {
        assert_eq!(foreground_window_rect(), None);
    }
}
