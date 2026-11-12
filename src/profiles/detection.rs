//! Foreground process detection.
//!
//! Non-invasive: only uses GetForegroundWindow, GetWindowThreadProcessId,
//! OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) and QueryFullProcessImageNameW.
//! No memory access, injection or hooking. Non-Windows builds return nothing.

use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// Lower and upper bounds for the polling interval, in milliseconds.
pub const MIN_INTERVAL_MS: u32 = 250;
pub const MAX_INTERVAL_MS: u32 = 5000;

/// A running process, as listed by [`list_running_processes`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RunningExe {
    /// Lowercase file name, e.g. "cs2.exe".
    pub name: String,
    /// Full image path as reported by Windows.
    pub path: String,
}

/// Events produced by the [`Detector`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionEvent {
    /// The foreground executable changed. `None` means no identifiable app is in front.
    Changed(Option<String>),
}

/// Result of inspecting the foreground window once.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Foreground {
    /// Lowercase file name of the foreground executable.
    Exe(String),
    /// OpenCrosshair itself is in front; its state should not be touched.
    Own,
    /// No foreground window, or its process could not be inspected.
    Unknown,
}

/// Lowercase file name of the executable in the foreground window, e.g. "cs2.exe".
///
/// Returns `None` when there is no foreground window, when the foreground window belongs to
/// OpenCrosshair itself, when its process cannot be opened, or on non-Windows platforms.
#[allow(dead_code)]
pub fn foreground_exe() -> Option<String> {
    match current_foreground() {
        Foreground::Exe(name) => Some(name),
        Foreground::Own | Foreground::Unknown => None,
    }
}

/// Running processes with an accessible image path, deduplicated by path and sorted by name.
/// OpenCrosshair's own process is excluded. Returns an empty list on non-Windows platforms.
pub fn list_running_processes() -> Vec<RunningExe> {
    platform::list_running_processes()
}

fn current_foreground() -> Foreground {
    platform::current_foreground()
}

/// Shared between the detector handle and its worker thread.
struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

struct State {
    enabled: bool,
    interval_ms: u32,
    shutdown: bool,
    /// Bumped whenever a setting changes so a sleeping worker re-evaluates immediately.
    epoch: u64,
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Background foreground-window watcher.
///
/// While disabled the worker thread blocks on a condition variable (no polling, no CPU use).
/// Events are sent only when the foreground executable changes. Disabling resets the
/// detector's notion of the current app, so after re-enabling a game that is still in front
/// is reported again. Consumers should clear their own state when disabling.
pub struct Detector {
    shared: Arc<Shared>,
    rx: Receiver<DetectionEvent>,
    join: Option<JoinHandle<()>>,
}

impl Detector {
    /// Start the worker thread (initially disabled). `repaint` is called after every event so
    /// the UI can wake up; pass a closure that calls `egui::Context::request_repaint`.
    pub fn spawn(repaint: Box<dyn Fn() + Send>) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                enabled: false,
                interval_ms: 1000,
                shutdown: false,
                epoch: 0,
            }),
            cv: Condvar::new(),
        });
        let (tx, rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let join = std::thread::Builder::new()
            .name("foreground-detector".into())
            .spawn(move || run_worker(&worker_shared, &tx, repaint.as_ref()))
            .ok();
        Self { shared, rx, join }
    }

    /// Enable or disable polling. Disabling makes the worker block until re-enabled.
    pub fn set_enabled(&self, enabled: bool) {
        let mut st = lock(&self.shared.state);
        if st.enabled != enabled {
            st.enabled = enabled;
            st.epoch = st.epoch.wrapping_add(1);
            self.shared.cv.notify_all();
        }
    }

    /// Set the polling interval; clamped to 250..=5000 ms.
    pub fn set_interval_ms(&self, ms: u32) {
        let ms = ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS);
        let mut st = lock(&self.shared.state);
        if st.interval_ms != ms {
            st.interval_ms = ms;
            st.epoch = st.epoch.wrapping_add(1);
            self.shared.cv.notify_all();
        }
    }

    /// Non-blocking receive of the next pending event.
    pub fn try_recv(&self) -> Option<DetectionEvent> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Detector {
    fn drop(&mut self) {
        {
            let mut st = lock(&self.shared.state);
            st.shutdown = true;
            st.epoch = st.epoch.wrapping_add(1);
            self.shared.cv.notify_all();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_worker(shared: &Shared, tx: &mpsc::Sender<DetectionEvent>, repaint: &dyn Fn()) {
    // `None` means "no identifiable app in front" and is also the state after a disable.
    let mut last: Option<String> = None;
    loop {
        let (interval_ms, epoch) = {
            let mut st = lock(&shared.state);
            while !st.enabled && !st.shutdown {
                st = shared.cv.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
            if st.shutdown {
                return;
            }
            (st.interval_ms, st.epoch)
        };

        let current = match current_foreground() {
            Foreground::Exe(name) => Some(name),
            Foreground::Unknown => None,
            Foreground::Own => last.clone(),
        };
        if current != last {
            last = current.clone();
            if tx.send(DetectionEvent::Changed(current)).is_ok() {
                repaint();
            }
        }

        // Sleep one interval, waking early on any setting change or shutdown.
        let st = lock(&shared.state);
        let _ = shared
            .cv
            .wait_timeout_while(st, Duration::from_millis(u64::from(interval_ms)), |s| {
                !s.shutdown && s.enabled && s.epoch == epoch
            })
            .unwrap_or_else(PoisonError::into_inner);

        let st = lock(&shared.state);
        if st.shutdown {
            return;
        }
        if !st.enabled {
            // Disabled: reset so a game still in front is reported after re-enabling.
            last = None;
        }
    }
}

/// Lowercase file name from a full path (handles both `\\` and `/`).
fn file_name_lower(path: &str) -> Option<String> {
    let name = path.rsplit(['\\', '/']).next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_ascii_lowercase())
    }
}

#[cfg(windows)]
mod platform {
    use super::{Foreground, RunningExe, file_name_lower};
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::ProcessStatus::K32EnumProcesses;
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    /// Image path of `pid`, or None if the process cannot be opened (e.g. protected/elevated).
    /// The handle is always closed before returning.
    fn process_image_path(pid: u32) -> Option<String> {
        // SAFETY: OpenProcess returns a handle we own; it is closed on every path below.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; 1024];
            let mut len = buf.len() as u32;
            let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(handle);
            result.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    pub fn current_foreground() -> Foreground {
        // SAFETY: plain query calls with no pointers that outlive the call.
        unsafe {
            let hwnd: HWND = GetForegroundWindow();
            if hwnd.0.is_null() {
                return Foreground::Unknown;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return Foreground::Unknown;
            }
            if pid == GetCurrentProcessId() {
                return Foreground::Own;
            }
            match process_image_path(pid).and_then(|p| file_name_lower(&p)) {
                Some(name) => Foreground::Exe(name),
                None => Foreground::Unknown,
            }
        }
    }

    pub fn list_running_processes() -> Vec<RunningExe> {
        let mut pids = vec![0u32; 4096];
        let mut needed = 0u32;
        // SAFETY: buffer is valid for `cb` bytes; `needed` receives the byte count written.
        let ok = unsafe {
            K32EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut needed,
            )
        };
        if ok.0 == 0 {
            return Vec::new();
        }
        let count = (needed as usize / std::mem::size_of::<u32>()).min(pids.len());
        let own = unsafe { GetCurrentProcessId() };

        let mut out: Vec<RunningExe> = Vec::new();
        for &pid in &pids[..count] {
            if pid == 0 || pid == own {
                continue;
            }
            let Some(path) = process_image_path(pid) else { continue };
            let Some(name) = file_name_lower(&path) else { continue };
            let entry = RunningExe { name, path };
            if !out.iter().any(|e| e.path.eq_ignore_ascii_case(&entry.path)) {
                out.push(entry);
            }
        }
        out.sort();
        out
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Foreground, RunningExe};

    pub fn current_foreground() -> Foreground {
        Foreground::Unknown
    }

    pub fn list_running_processes() -> Vec<RunningExe> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_lower_handles_windows_and_unix_paths() {
        assert_eq!(file_name_lower(r"C:\Games\CS2\CS2.EXE").as_deref(), Some("cs2.exe"));
        assert_eq!(file_name_lower("/opt/game/Game.exe").as_deref(), Some("game.exe"));
        assert_eq!(file_name_lower("plain.exe").as_deref(), Some("plain.exe"));
        assert_eq!(file_name_lower("C:\\dir\\"), None);
        assert_eq!(file_name_lower(""), None);
    }

    #[test]
    fn interval_is_clamped_by_setters() {
        let det = Detector::spawn(Box::new(|| {}));
        det.set_interval_ms(1);
        assert_eq!(lock(&det.shared.state).interval_ms, MIN_INTERVAL_MS);
        det.set_interval_ms(999_999);
        assert_eq!(lock(&det.shared.state).interval_ms, MAX_INTERVAL_MS);
    }

    #[test]
    fn detector_stays_silent_while_disabled_and_joins_on_drop() {
        let det = Detector::spawn(Box::new(|| {}));
        std::thread::sleep(Duration::from_millis(50));
        assert!(det.try_recv().is_none());
        drop(det); // must not hang
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_detection_is_empty() {
        assert_eq!(foreground_exe(), None);
        assert!(list_running_processes().is_empty());
    }
}
