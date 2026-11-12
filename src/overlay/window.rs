//! Overlay thread + layered window (agent). Must expose:
use super::OverlayCommand;
use std::sync::mpsc::Receiver;

/// Windows: spawn thread running Win32 message loop and handling commands.
/// Other OS: return None and drain nothing.
pub fn spawn_thread(_rx: Receiver<OverlayCommand>) -> Option<std::thread::JoinHandle<()>> {
    None
}
