//! Overlay thread + layered window.
//!
//! # Win32 design (normal, documented window mechanisms only)
//!
//! - Window: `WS_POPUP` layered window (`WS_EX_LAYERED`), presented with `UpdateLayeredWindow`
//!   (`ULW_ALPHA`) from a 32-bit premultiplied top-down DIB. Per-pixel alpha, no color key.
//! - Click-through: `WS_EX_TRANSPARENT` makes hit testing fall through to the game.
//! - Focus: `WS_EX_NOACTIVATE` plus `SW_SHOWNOACTIVATE` and `SWP_NOACTIVATE`, so the overlay
//!   never takes foreground or keyboard focus.
//! - Taskbar / Alt+Tab: `WS_EX_TOOLWINDOW` keeps it out of both.
//! - Z-order: `WS_EX_TOPMOST` plus `HWND_TOPMOST`, re-asserted only when we reposition or
//!   change visibility. There is no polling for z-order.
//! - DPI: the process is opted into per-monitor-aware-v2 (`init_dpi_awareness`). All sizes and
//!   positions are physical pixels; crosshair offsets are scaled by the target monitor's DPI.
//! - Threading: one thread owns the window, DCs and DIBs. It blocks in `GetMessageW` and
//!   wakes only for: queued commands (`WM_APP_COMMAND`, posted by [`wake`]), display or DPI
//!   changes (`WM_DISPLAYCHANGE`/`WM_DPICHANGED` via the window procedure), foreground or
//!   foreground-location events (WinEvent hooks), and an optional 500 ms timer that exists only
//!   when those hooks could not be installed and the mode is `GameWindow`. No sleeps or busy loops.
//! - Rendering is cached: the bitmap is re-rendered only when the crosshair or the DPI scale
//!   changes. Hidden overlays do no rendering or positioning work.
//! - Cleanup: on shutdown the hooks are removed, the timer is killed, the DIB is deselected
//!   before deletion, the DC and bitmap are deleted, and the window and class are destroyed.
//!
//! # Hooks
//!
//! The only hooks are two out-of-context WinEvent hooks (`WINEVENT_OUTOFCONTEXT`) on
//! `EVENT_SYSTEM_FOREGROUND` and `EVENT_OBJECT_LOCATIONCHANGE` (filtered to the foreground window).
//! They are documented accessibility events. Nothing is injected into other processes.
//!
//! # Limitations
//!
//! - Exclusive-fullscreen DirectX/Vulkan games bypass DWM composition, so a layered window
//!   cannot be drawn over them. Borderless or windowed fullscreen works.
//! - Windows does not guarantee that a topmost window stays above every application. Some
//!   fullscreen shells and protected surfaces (UAC secure desktop, some anti-cheat overlays)
//!   can cover it. This is an OS limit, not something this module can work around.
//! - `GameWindow` mode follows the foreground window. Windows of this process are ignored so
//!   the settings UI does not become the anchor; the monitor center is used instead.

use super::OverlayCommand;
use std::sync::mpsc::Receiver;

/// Convert premultiplied RGBA (renderer output) to premultiplied BGRA (DIB byte order).
/// Returns false and writes nothing if the buffers are mismatched or not whole pixels.
pub fn premultiplied_rgba_to_bgra(src: &[u8], dst: &mut [u8]) -> bool {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return false;
    }
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = s[3];
    }
    true
}

/// Wake the overlay thread so it drains queued commands. Called after each command is sent.
pub fn wake() {
    #[cfg(windows)]
    imp::wake();
}

/// Windows: spawn the thread running the Win32 message loop and handling commands.
/// Other OS: return None; the receiver is dropped and commands are discarded.
#[cfg(windows)]
pub fn spawn_thread(rx: Receiver<OverlayCommand>) -> Option<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("opencrosshair-overlay".into())
        .spawn(move || imp::run(rx))
        .map_err(|e| tracing::error!("failed to spawn overlay thread: {e}"))
        .ok()
}

/// Other OS: no overlay thread.
#[cfg(not(windows))]
pub fn spawn_thread(_rx: Receiver<OverlayCommand>) -> Option<std::thread::JoinHandle<()>> {
    None
}

#[cfg(windows)]
mod imp {
    use super::super::OverlayCommand;
    use super::super::positioning::{self, compute_position_scaled, dpi_scale_for};
    use super::super::windows_api::{
        self, HookHandle, Surface, WM_APP_COMMAND, WM_APP_DISPLAY_CHANGED, WM_APP_GAME_WINDOW,
    };
    use super::premultiplied_rgba_to_bgra;
    use crate::crosshair::model::Crosshair;
    use crate::crosshair::renderer::render;
    use crate::storage::config::{CenterMode, PositionSettings};
    use crate::system::monitors::{MonitorInfo, list_monitors};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{Receiver, TryRecvError};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, OBJID_WINDOW,
        TranslateMessage, WM_TIMER,
    };

    /// Timer id used only in the fallback path (hooks unavailable, GameWindow mode).
    const GAME_TIMER_ID: usize = 1;
    const GAME_TIMER_MS: u32 = 500;

    pub fn wake() {
        windows_api::post_to_overlay(WM_APP_COMMAND);
    }

    unsafe extern "system" fn foreground_proc(
        _hook: HookHandle,
        _event: u32,
        _hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        windows_api::post_to_overlay(WM_APP_GAME_WINDOW);
    }

    unsafe extern "system" fn location_proc(
        _hook: HookHandle,
        _event: u32,
        hwnd: HWND,
        id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        // Only the foreground window's own location changes matter.
        if id_object == OBJID_WINDOW.0 && windows_api::foreground_hwnd() == Some(hwnd) {
            windows_api::post_to_overlay(WM_APP_GAME_WINDOW);
        }
    }

    struct Overlay {
        hwnd: HWND,
        crosshair: Crosshair,
        position: PositionSettings,
        visible: bool,
        monitors: Vec<MonitorInfo>,
        surface: Option<Surface>,
        /// Crosshair and DPI scale the current surface was rendered for.
        rendered: Option<(Crosshair, f32)>,
        hooks: Vec<HookHandle>,
        timer_active: bool,
    }

    impl Overlay {
        fn create() -> Result<Self, String> {
            windows_api::register_window_class()?;
            let hwnd = match windows_api::create_overlay_window() {
                Ok(h) => h,
                Err(e) => {
                    windows_api::unregister_window_class();
                    return Err(e);
                }
            };
            let mut hooks = Vec::new();
            if let Some(h) = windows_api::install_win_event_hook(EVENT_SYSTEM_FOREGROUND, Some(foreground_proc)) {
                hooks.push(h);
            } else {
                tracing::warn!("foreground WinEvent hook unavailable; using timer fallback in GameWindow mode");
            }
            if let Some(h) = windows_api::install_win_event_hook(EVENT_OBJECT_LOCATIONCHANGE, Some(location_proc)) {
                hooks.push(h);
            } else {
                tracing::warn!("location WinEvent hook unavailable; using timer fallback in GameWindow mode");
            }
            Ok(Self {
                hwnd,
                crosshair: Crosshair::default(),
                position: PositionSettings::default(),
                visible: false,
                monitors: list_monitors(),
                surface: None,
                rendered: None,
                hooks,
                timer_active: false,
            })
        }

        /// Re-render only if the crosshair or DPI scale differs from the cached surface.
        fn ensure_surface(&mut self, scale: f32) -> Result<(), String> {
            if self.surface.is_some()
                && self
                    .rendered
                    .as_ref()
                    .is_some_and(|(c, s)| *c == self.crosshair && *s == scale)
            {
                return Ok(());
            }
            let bmp = render(&self.crosshair, scale);
            if bmp.width == 0 || bmp.height == 0 || bmp.pixels.len() != bmp.width as usize * bmp.height as usize * 4 {
                return Err("renderer returned a malformed bitmap".into());
            }
            let reusable = self
                .surface
                .as_ref()
                .is_some_and(|s| s.width == bmp.width && s.height == bmp.height);
            if !reusable {
                self.rendered = None;
                self.surface = None;
                self.surface = Some(Surface::new(bmp.width, bmp.height)?);
            }
            let surface = self.surface.as_mut().ok_or("surface missing")?;
            if !premultiplied_rgba_to_bgra(&bmp.pixels, surface.pixels_mut()) {
                return Err("pixel conversion size mismatch".into());
            }
            self.rendered = Some((self.crosshair.clone(), scale));
            Ok(())
        }

        /// Position and present the surface. Does nothing while hidden.
        fn refresh(&mut self) {
            if !self.visible {
                return;
            }
            let game_rect = match self.position.mode {
                CenterMode::GameWindow => positioning::foreground_window_rect(),
                _ => None,
            };
            let scale = dpi_scale_for(&self.monitors, game_rect, &self.position);
            if let Err(e) = self.ensure_surface(scale) {
                tracing::warn!("overlay render failed: {e}");
                return;
            }
            let Some(surface) = self.surface.as_ref() else { return };
            let (x, y) = compute_position_scaled(
                &self.monitors,
                game_rect,
                &self.position,
                &self.crosshair,
                surface.width,
                surface.height,
                scale,
            );
            if let Err(e) = windows_api::update_layered(self.hwnd, x, y, surface) {
                tracing::warn!("overlay present failed: {e}");
                return;
            }
            windows_api::set_topmost(self.hwnd);
        }

        fn set_visible(&mut self, visible: bool) {
            self.visible = visible;
            if visible {
                // Content and position first, so the first visible frame is correct.
                self.refresh();
                windows_api::show_no_activate(self.hwnd, true);
            } else {
                windows_api::show_no_activate(self.hwnd, false);
            }
        }

        fn sync_timer(&mut self) {
            let fallback = self.hooks.len() < 2;
            let want = fallback && self.visible && self.position.mode == CenterMode::GameWindow;
            if want && !self.timer_active {
                self.timer_active = windows_api::start_thread_timer(GAME_TIMER_ID, GAME_TIMER_MS);
            } else if !want && self.timer_active {
                windows_api::stop_thread_timer(GAME_TIMER_ID);
                self.timer_active = false;
            }
        }

        fn on_game_event(&mut self) {
            if self.position.mode == CenterMode::GameWindow {
                self.refresh();
            }
        }

        /// Apply every queued command, coalescing into one refresh. Returns false to stop.
        fn drain(&mut self, rx: &Receiver<OverlayCommand>) -> bool {
            let mut keep_running = true;
            let mut dirty = false;
            let mut visibility: Option<bool> = None;
            loop {
                match rx.try_recv() {
                    Ok(OverlayCommand::Update { crosshair, position }) => {
                        self.crosshair = crosshair;
                        self.position = position;
                        dirty = true;
                    }
                    Ok(OverlayCommand::Reposition(position)) => {
                        self.position = position;
                        dirty = true;
                    }
                    Ok(OverlayCommand::SetVisible(v)) => visibility = Some(v),
                    Ok(OverlayCommand::Shutdown) | Err(TryRecvError::Disconnected) => {
                        keep_running = false;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            match visibility {
                Some(v) if v != self.visible => self.set_visible(v),
                _ if dirty => self.refresh(),
                _ => {}
            }
            self.sync_timer();
            keep_running
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            if self.timer_active {
                windows_api::stop_thread_timer(GAME_TIMER_ID);
            }
            for h in self.hooks.drain(..) {
                windows_api::unhook_win_event(h);
            }
            // Surface releases its GDI objects before the window goes away.
            self.surface = None;
            windows_api::destroy_window(self.hwnd);
            windows_api::unregister_window_class();
        }
    }

    pub fn run(rx: Receiver<OverlayCommand>) {
        windows_api::init_dpi_awareness();
        windows_api::prepare_thread_queue();
        // Publish the thread id before creating anything that posts to it.
        windows_api::OVERLAY_THREAD_ID.store(windows_api::current_thread_id(), Ordering::SeqCst);
        let mut overlay = match Overlay::create() {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("overlay window unavailable: {e}");
                windows_api::OVERLAY_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            }
        };
        // Commands sent before this thread started waiting must not be stranded in the channel.
        let mut running = overlay.drain(&rx);
        while running {
            let mut msg = MSG::default();
            // SAFETY: msg is a valid out-parameter; blocks until a message arrives.
            let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if got.0 <= 0 {
                break; // WM_QUIT or error
            }
            match msg.message {
                WM_APP_COMMAND => running = overlay.drain(&rx),
                WM_APP_DISPLAY_CHANGED => {
                    overlay.monitors = list_monitors();
                    overlay.refresh();
                }
                WM_APP_GAME_WINDOW => overlay.on_game_event(),
                WM_TIMER if msg.wParam.0 == GAME_TIMER_ID => overlay.on_game_event(),
                _ => {
                    // SAFETY: standard dispatch; WinEvent hook callbacks arrive through here.
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
        windows_api::OVERLAY_THREAD_ID.store(0, Ordering::SeqCst);
        drop(overlay);
    }
}

#[cfg(test)]
mod tests {
    use super::premultiplied_rgba_to_bgra;

    #[test]
    fn swaps_red_and_blue_keeping_alpha() {
        let src = [10u8, 20, 30, 40, 1, 2, 3, 4];
        let mut dst = [0u8; 8];
        assert!(premultiplied_rgba_to_bgra(&src, &mut dst));
        assert_eq!(dst, [30, 20, 10, 40, 3, 2, 1, 4]);
    }

    #[test]
    fn rejects_mismatched_or_partial_pixels() {
        let src = [0u8; 8];
        let mut short = [0u8; 4];
        assert!(!premultiplied_rgba_to_bgra(&src, &mut short));
        let src3 = [0u8; 6];
        let mut dst3 = [0u8; 6];
        assert!(!premultiplied_rgba_to_bgra(&src3, &mut dst3));
    }
}
