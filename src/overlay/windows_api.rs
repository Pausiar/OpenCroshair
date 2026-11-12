//! Thin unsafe Win32 wrappers for the overlay (cfg(windows) only).
//!
//! Everything here uses documented, non-invasive APIs: window classes, layered windows,
//! DIB sections, WinEvent out-of-context hooks, and thread messages. No injection,
//! no hooking of other processes, no memory access.
#![cfg(windows)]

use std::ffi::c_void;
use std::sync::Once;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    HWND_TOPMOST, IsIconic, IsWindowVisible, KillTimer, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    RegisterClassExW, SET_WINDOW_POS_FLAGS, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetTimer, SetWindowPos, ShowWindow, ULW_ALPHA, UnregisterClassW, UpdateLayeredWindow, WM_APP, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// Opaque `HWINEVENTHOOK`. Declared locally because the `Win32_UI_Accessibility` feature (which
/// provides the crate's binding) is not enabled in Cargo.toml. ABI-identical: pointer-sized,
/// transparent over `*mut c_void`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookHandle(pub *mut c_void);

impl HookHandle {
    fn is_invalid(self) -> bool {
        self.0.is_null()
    }
}

/// `WINEVENTPROC`.
pub type WinEventProc = Option<unsafe extern "system" fn(HookHandle, u32, HWND, i32, i32, u32, u32)>;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWinEventHook(
        eventmin: u32,
        eventmax: u32,
        hmodwineventproc: *mut c_void,
        pfnwineventproc: WinEventProc,
        idprocess: u32,
        idthread: u32,
        dwflags: u32,
    ) -> HookHandle;
    fn UnhookWinEvent(hwineventhook: HookHandle) -> windows::core::BOOL;
}

/// Commands were queued on the mpsc channel; drain it.
pub const WM_APP_COMMAND: u32 = WM_APP + 1;
/// Display configuration or DPI changed: re-enumerate monitors and reposition.
pub const WM_APP_DISPLAY_CHANGED: u32 = WM_APP + 2;
/// Foreground window changed or moved: reposition if in GameWindow mode.
pub const WM_APP_GAME_WINDOW: u32 = WM_APP + 3;

/// Thread id of the overlay thread (0 when no overlay thread is running). Used by the window
/// procedure and WinEvent callbacks, which have no other way to reach the thread.
pub static OVERLAY_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
static DPI_INIT: Once = Once::new();

const CLASS_NAME: PCWSTR = w!("OpenCrosshairOverlay");

/// Opt the process into per-monitor-aware-v2 DPI so every coordinate we see is physical.
/// Idempotent. If the host already set an awareness mode, the call fails harmlessly.
pub fn init_dpi_awareness() {
    DPI_INIT.call_once(|| {
        // SAFETY: plain process-level setting, no pointers involved.
        if let Err(e) = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) } {
            tracing::debug!("DPI awareness already set by host: {e}");
        }
    });
}

/// Create this thread's message queue so PostThreadMessage can target it.
pub fn prepare_thread_queue() {
    let mut msg = MSG::default();
    // SAFETY: PM_NOREMOVE only creates the queue; nothing is dequeued.
    unsafe {
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    }
}

pub fn current_thread_id() -> u32 {
    // SAFETY: no preconditions.
    unsafe { GetCurrentThreadId() }
}

/// Post a thread message to the overlay thread if it is running.
pub fn post_to_overlay(msg: u32) {
    let tid = OVERLAY_THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        // SAFETY: posting to a thread id; failure (thread gone) is harmless.
        let _ = unsafe { PostThreadMessageW(tid, msg, WPARAM(0), LPARAM(0)) };
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DISPLAYCHANGE || msg == WM_DPICHANGED {
        // Handled on the overlay thread; we deliberately ignore the suggested rect in lParam
        // because the overlay is placed by our own positioning logic.
        post_to_overlay(WM_APP_DISPLAY_CHANGED);
    }
    // SAFETY: forwarding the unmodified arguments to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Register the overlay window class (idempotent per process lifetime; unregistered on drop).
pub fn register_window_class() -> Result<(), String> {
    // SAFETY: GetModuleHandleW(None) returns the current executable's module handle.
    let hmod = unsafe { GetModuleHandleW(None) }.map_err(|e| format!("GetModuleHandleW failed: {e}"))?;
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE::from(hmod),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    // SAFETY: wc is fully initialised and outlives the call; the name is a static wide string.
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err("RegisterClassExW failed".into());
    }
    Ok(())
}

pub fn unregister_window_class() {
    // SAFETY: the class was registered by this module with this module's instance.
    if let Ok(hmod) = unsafe { GetModuleHandleW(None) } {
        let _ = unsafe { UnregisterClassW(CLASS_NAME, Some(HINSTANCE::from(hmod))) };
    }
}

/// Create the hidden, click-through, never-activating layered popup used as the crosshair.
///
/// Style rationale:
/// - WS_POPUP: no caption/border, no owner relationship that could drag it with another window.
/// - WS_EX_LAYERED: required for UpdateLayeredWindow per-pixel alpha.
/// - WS_EX_TRANSPARENT: hit testing falls through, so mouse input reaches the game.
/// - WS_EX_NOACTIVATE: never receives focus or activation, so keyboard input stays in the game.
/// - WS_EX_TOOLWINDOW: no taskbar button and no Alt+Tab entry.
/// - WS_EX_TOPMOST: z-order hint; re-asserted with HWND_TOPMOST on each reposition.
pub fn create_overlay_window() -> Result<HWND, String> {
    // SAFETY: plain window creation on the calling (overlay) thread.
    let hmod = unsafe { GetModuleHandleW(None) }.map_err(|e| format!("GetModuleHandleW failed: {e}"))?;
    let ex_style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            CLASS_NAME,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(HINSTANCE::from(hmod)),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW failed: {e}"))?;
    OVERLAY_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    Ok(hwnd)
}

pub fn destroy_window(hwnd: HWND) {
    OVERLAY_HWND.store(0, Ordering::SeqCst);
    // SAFETY: hwnd was created by create_overlay_window on this thread.
    let _ = unsafe { DestroyWindow(hwnd) };
}

/// Show or hide without activating. Visibility transitions only; never called in a loop.
pub fn show_no_activate(hwnd: HWND, visible: bool) {
    let cmd = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
    // SAFETY: hwnd is a valid window owned by this thread.
    unsafe {
        let _ = ShowWindow(hwnd, cmd);
    }
}

/// Re-assert HWND_TOPMOST without moving, resizing or activating.
pub fn set_topmost(hwnd: HWND) {
    let flags: SET_WINDOW_POS_FLAGS = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    // SAFETY: hwnd is valid; flags only change z-order.
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
    }
}

/// Present `surface` at (x,y) in physical virtual-desktop coordinates with per-pixel alpha.
pub fn update_layered(hwnd: HWND, x: i32, y: i32, surface: &Surface) -> Result<(), String> {
    let dst = POINT { x, y };
    let size = SIZE {
        cx: surface.width as i32,
        cy: surface.height as i32,
    };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    // SAFETY: all pointers reference locals that live for the duration of the call; the
    // source DC holds the selected DIB owned by `surface`.
    unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            Some(&dst as *const POINT),
            Some(&size as *const SIZE),
            Some(surface.dc),
            Some(&src as *const POINT),
            COLORREF(0),
            Some(&blend as *const BLENDFUNCTION),
            ULW_ALPHA,
        )
    }
    .map_err(|e| format!("UpdateLayeredWindow failed: {e}"))
}

/// A 32-bit top-down DIB selected into a memory DC. Owns all GDI objects it creates.
pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
    pub width: u32,
    pub height: u32,
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("surface size must be non-zero".into());
        }
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Negative height = top-down rows, matching the renderer's output order.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        // SAFETY: bmi is valid for the call; on success `bits` points to width*height*4 bytes
        // owned by the section until DeleteObject.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err("CreateCompatibleDC failed".into());
            }
            let bitmap = match CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(b) => b,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    return Err(format!("CreateDIBSection failed: {e}"));
                }
            };
            if bits.is_null() {
                let _ = DeleteObject(HGDIOBJ::from(bitmap));
                let _ = DeleteDC(dc);
                return Err("CreateDIBSection returned no pixel buffer".into());
            }
            let previous = SelectObject(dc, HGDIOBJ::from(bitmap));
            Ok(Self {
                dc,
                bitmap,
                previous,
                bits: bits as *mut u8,
                width,
                height,
            })
        }
    }

    /// Mutable BGRA pixel storage (len = width*height*4), top-down.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        let len = self.width as usize * self.height as usize * 4;
        // SAFETY: `bits` points to a section of exactly this size, valid while self lives, and
        // &mut self guarantees exclusive access.
        unsafe { std::slice::from_raw_parts_mut(self.bits, len) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: the DIB must be deselected before it or the DC can be deleted; we restore
        // the DC's original object first, as GDI requires.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ::from(self.bitmap));
            let _ = DeleteDC(self.dc);
        }
    }
}

pub fn foreground_hwnd() -> Option<HWND> {
    // SAFETY: no preconditions.
    let h = unsafe { GetForegroundWindow() };
    (!h.is_invalid()).then_some(h)
}

pub fn window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut r = RECT::default();
    // SAFETY: r is a valid out-parameter.
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    Some((r.left, r.top, r.right, r.bottom))
}

pub fn is_iconic(hwnd: HWND) -> bool {
    // SAFETY: query only.
    unsafe { IsIconic(hwnd) }.as_bool()
}

pub fn is_visible(hwnd: HWND) -> bool {
    // SAFETY: query only.
    unsafe { IsWindowVisible(hwnd) }.as_bool()
}

pub fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: pid is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32)) };
    pid
}

pub fn current_pid() -> u32 {
    // SAFETY: no preconditions.
    unsafe { GetCurrentProcessId() }
}

pub fn is_overlay_window(hwnd: HWND) -> bool {
    let ours = OVERLAY_HWND.load(Ordering::SeqCst);
    ours != 0 && hwnd.0 as isize == ours
}

/// Install an out-of-context WinEvent hook delivered to the calling thread's message queue.
/// Returns None on failure.
pub fn install_win_event_hook(event: u32, proc: WinEventProc) -> Option<HookHandle> {
    const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
    const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;
    // SAFETY: out-of-context hooks take no pointers from us; events arrive as messages.
    let h = unsafe {
        SetWinEventHook(
            event,
            event,
            std::ptr::null_mut(),
            proc,
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    (!h.is_invalid()).then_some(h)
}

pub fn unhook_win_event(h: HookHandle) {
    // SAFETY: h came from install_win_event_hook on this thread.
    let _ = unsafe { UnhookWinEvent(h) };
}

/// Start a timer delivered as WM_TIMER to this thread (used only if hooks are unavailable).
pub fn start_thread_timer(id: usize, interval_ms: u32) -> bool {
    // SAFETY: hwnd None = thread timer; no callback.
    unsafe { SetTimer(None, id, interval_ms, None) != 0 }
}

pub fn stop_thread_timer(id: usize) {
    // SAFETY: id was created by start_thread_timer on this thread.
    let _ = unsafe { KillTimer(None, id) };
}
