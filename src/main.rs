// Release builds are GUI-only: no console window. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unknown_lints)]
// Newer clippy suggests `as_chunks`, which is not stable on older toolchains.
#![allow(clippy::chunks_exact_to_as_chunks)]
mod app;
mod crosshair;
mod hotkeys;
mod overlay;
mod profiles;
mod state;
mod storage;
mod system;
mod ui;

use storage::config::Paths;

/// Keeps the named mutex alive for the lifetime of the process (Windows only).
#[cfg(windows)]
mod single_instance {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::w;

    pub struct Guard(#[allow(dead_code)] HANDLE);

    /// `None` when another instance already holds the mutex.
    pub fn acquire() -> Option<Guard> {
        // SAFETY: plain Win32 call with a static, NUL-terminated name and no security attributes.
        unsafe {
            let handle = CreateMutexW(None, true, w!("Local\\OpenCrosshair.SingleInstance")).ok()?;
            if windows::core::Error::from_thread().code() == ERROR_ALREADY_EXISTS.to_hresult() {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Guard(handle))
        }
    }
}

fn app_icon() -> eframe::egui::IconData {
    // 256x256 application icon; fall back to the procedural icon if decoding ever fails.
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/icon.png")).unwrap_or_else(|_| {
        let (rgba, width, height) = system::tray::icon_rgba();
        eframe::egui::IconData { rgba, width, height }
    })
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    let _instance = match single_instance::acquire() {
        Some(guard) => guard,
        None => return Ok(()),
    };

    let launched_minimized = std::env::args().any(|a| a == "--minimized");
    let paths = Paths::resolve().unwrap_or_else(|| Paths::from_root(std::env::temp_dir().join("OpenCrosshair")));

    let logging_warning = match storage::init_logging(&paths) {
        Ok(path) => {
            tracing::info!(
                "OpenCrosshair {} starting (log: {})",
                env!("CARGO_PKG_VERSION"),
                path.display()
            );
            None
        }
        Err(e) => Some(format!("Logging is disabled: {e}")),
    };

    let mut data = storage::config::load_all(&paths);
    if let Some(w) = logging_warning {
        data.warnings.push(w);
    }
    let start_hidden = launched_minimized || data.settings.start_minimized;

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("OpenCrosshair")
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([900.0, 600.0])
        .with_icon(app_icon())
        // The custom title bar (app.rs) replaces the OS frame unless the user opted into the system one.
        .with_decorations(data.settings.use_system_title_bar)
        .with_visible(!start_hidden);
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "OpenCrosshair",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, paths, data, start_hidden)))),
    )
}
