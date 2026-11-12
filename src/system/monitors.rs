//! Monitor enumeration. Windows: EnumDisplayMonitors + GetMonitorInfoW + GetDpiForMonitor.
//! Other OS: a single 1920x1080 primary display so the rest of the app still works.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    /// Physical pixel rect of the monitor on the virtual desktop.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    /// DPI scale factor (1.0 = 96dpi).
    pub scale: f32,
}

/// Raw monitor data as reported by the OS, before ordering and naming.
#[derive(Debug, Clone, PartialEq)]
pub struct RawMonitor {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    pub scale: f32,
}

/// Orders monitors (primary first, then left-to-right, top-to-bottom), assigns indices and
/// names them "Display N (WxH)". Indices are the values stored in `PositionSettings::monitor`.
pub fn build_monitor_list(mut raw: Vec<RawMonitor>) -> Vec<MonitorInfo> {
    raw.sort_by_key(|m| (!m.primary, m.x, m.y));
    raw.into_iter()
        .enumerate()
        .map(|(index, m)| MonitorInfo {
            index,
            name: format!("Display {} ({}x{})", index + 1, m.width, m.height),
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
            primary: m.primary,
            scale: if m.scale.is_finite() && m.scale > 0.0 {
                m.scale
            } else {
                1.0
            },
        })
        .collect()
}

/// Single 1920x1080 primary display, used when enumeration is unavailable or fails.
pub fn fallback_monitors() -> Vec<MonitorInfo> {
    build_monitor_list(vec![RawMonitor {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        primary: true,
        scale: 1.0,
    }])
}

/// Enumerate monitors, primary first.
#[cfg(windows)]
pub fn list_monitors() -> Vec<MonitorInfo> {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
    use windows::core::BOOL;

    unsafe extern "system" fn enum_proc(hmon: HMONITOR, _hdc: HDC, _rc: *mut RECT, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the `&mut Vec<RawMonitor>` passed by `list_monitors` below, alive for the call.
        let out = unsafe { &mut *(lparam.0 as *mut Vec<RawMonitor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: info is a valid MONITORINFOEXW whose cbSize marks the extended layout.
        if unsafe { GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            let rc = info.monitorInfo.rcMonitor;
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            // SAFETY: out-pointers reference live locals.
            let scale = if unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok() {
                dpi_x as f32 / 96.0
            } else {
                1.0
            };
            out.push(RawMonitor {
                x: rc.left,
                y: rc.top,
                width: rc.right - rc.left,
                height: rc.bottom - rc.top,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                scale,
            });
        }
        BOOL(1)
    }

    // Coordinates are only physical when the process is per-monitor DPI aware.
    crate::overlay::windows_api::init_dpi_awareness();
    let mut raw: Vec<RawMonitor> = Vec::new();
    // SAFETY: `raw` outlives the synchronous enumeration; enum_proc only touches it through lparam.
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut raw as *mut Vec<RawMonitor> as isize),
        )
    };
    if !ok.as_bool() || raw.is_empty() {
        tracing::warn!("monitor enumeration returned no displays; using fallback");
        return fallback_monitors();
    }
    build_monitor_list(raw)
}

/// Non-Windows fallback: one 1920x1080 primary display.
#[cfg(not(windows))]
pub fn list_monitors() -> Vec<MonitorInfo> {
    fallback_monitors()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(x: i32, y: i32, w: i32, h: i32, primary: bool) -> RawMonitor {
        RawMonitor {
            x,
            y,
            width: w,
            height: h,
            primary,
            scale: 1.0,
        }
    }

    #[test]
    fn primary_comes_first_then_left_to_right() {
        let list = build_monitor_list(vec![
            raw(-2560, 0, 2560, 1440, false),
            raw(0, 0, 1920, 1080, true),
            raw(1920, -200, 1920, 1080, false),
        ]);
        assert_eq!(list.len(), 3);
        assert!(list[0].primary);
        assert_eq!((list[0].x, list[0].y), (0, 0));
        assert_eq!((list[1].x, list[1].y), (-2560, 0));
        assert_eq!((list[2].x, list[2].y), (1920, -200));
        for (i, m) in list.iter().enumerate() {
            assert_eq!(m.index, i);
        }
    }

    #[test]
    fn names_are_one_based_with_resolution() {
        let list = build_monitor_list(vec![raw(0, 0, 2560, 1440, true), raw(2560, 0, 1920, 1080, false)]);
        assert_eq!(list[0].name, "Display 1 (2560x1440)");
        assert_eq!(list[1].name, "Display 2 (1920x1080)");
    }

    #[test]
    fn invalid_scale_becomes_one() {
        let mut bad = raw(0, 0, 800, 600, true);
        bad.scale = 0.0;
        let mut nan = raw(800, 0, 800, 600, false);
        nan.scale = f32::NAN;
        let list = build_monitor_list(vec![bad, nan]);
        assert_eq!(list[0].scale, 1.0);
        assert_eq!(list[1].scale, 1.0);
    }

    #[test]
    fn fallback_is_single_1920x1080_primary() {
        let list = fallback_monitors();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].width, 1920);
        assert_eq!(list[0].height, 1080);
        assert!(list[0].primary);
        assert_eq!(list[0].index, 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn list_monitors_off_windows_returns_fallback() {
        assert_eq!(list_monitors(), fallback_monitors());
    }
}
