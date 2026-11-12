//! Overlay: independent Win32 layered window on its own thread. Public API (contract):
use crate::crosshair::model::Crosshair;
use crate::storage::config::PositionSettings;

pub mod positioning;
pub mod window;
pub mod windows_api;

/// Commands sent from UI/hotkeys to the overlay thread.
#[derive(Debug, Clone)]
pub enum OverlayCommand {
    /// Replace crosshair + position and redraw.
    Update {
        crosshair: Crosshair,
        position: PositionSettings,
    },
    /// Show/hide window (overlay_enabled && crosshair_visible).
    SetVisible(bool),
    /// Re-position only (monitor / game window changed).
    #[allow(dead_code)] // Part of the contract; the UI currently sends Update, which also repositions.
    Reposition(PositionSettings),
    Shutdown,
}

/// Handle owned by App. Dropping it shuts the thread down. On non-Windows it is a no-op sink.
pub struct OverlayHandle {
    tx: std::sync::mpsc::Sender<OverlayCommand>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl OverlayHandle {
    /// Spawn the overlay thread (hidden until SetVisible(true)).
    pub fn spawn() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let join = window::spawn_thread(rx);
        Self { tx, join }
    }
    pub fn send(&self, cmd: OverlayCommand) {
        if self.tx.send(cmd).is_ok() {
            window::wake();
        }
    }
}

impl Drop for OverlayHandle {
    fn drop(&mut self) {
        if self.tx.send(OverlayCommand::Shutdown).is_ok() {
            window::wake();
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
