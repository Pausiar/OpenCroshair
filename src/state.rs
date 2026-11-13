//! Shared application state mutated by UI pages (contract).
use crate::crosshair::model::{Crosshair, Preset};
use crate::overlay::{OverlayCommand, OverlayHandle};
use crate::profiles::GameProfile;
use crate::storage::config::Settings;

/// Which page is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Overview,
    Editor,
    Presets,
    Profiles,
    Settings,
}

pub struct AppState {
    pub settings: Settings,
    pub presets: Vec<Preset>,
    pub profiles: Vec<GameProfile>,
    /// Work-in-progress crosshair edited in the Editor (applied with "Apply").
    pub editing: Crosshair,
    pub page: Page,
    /// Name of the profile currently auto-applied (shown in Overview).
    pub active_profile: Option<String>,
    /// Transient status message shown in a toast/status bar: (text, is_error).
    pub status: Option<(String, bool)>,
    /// Set whenever persisted data changed; app shell debounces + saves on a worker thread.
    pub dirty: bool,
    pub overlay: OverlayHandle,
    /// Last registration error per hotkey action name, shown in Settings > Hotkeys.
    pub hotkey_errors: Vec<(String, String)>,
    /// Set by Settings when hotkey strings changed so app re-registers.
    pub hotkeys_changed: bool,
    /// Set by Settings when startup toggle changed.
    pub startup_changed: bool,
}

impl AppState {
    pub fn toast(&mut self, msg: impl Into<String>, error: bool) {
        self.status = Some((msg.into(), error));
    }
    /// Turn the overlay on or off, push the new state to the overlay thread and mark settings dirty.
    pub fn set_overlay_enabled(&mut self, on: bool) {
        if self.settings.overlay_enabled != on {
            self.settings.overlay_enabled = on;
            self.push_overlay();
            self.dirty = true;
        }
    }
    /// Push the active crosshair + position + visibility to the overlay thread.
    pub fn push_overlay(&self) {
        self.overlay.send(OverlayCommand::Update {
            crosshair: self.settings.active_crosshair.clone(),
            position: self.settings.position.clone(),
        });
        self.overlay.send(OverlayCommand::SetVisible(
            self.settings.overlay_enabled && self.settings.crosshair_visible,
        ));
    }
    /// Make `preset` the active crosshair (also loads it into the editor), updates recents, pushes overlay.
    pub fn apply_preset(&mut self, id: &str) {
        if let Some(p) = self.presets.iter().find(|p| p.id == id).cloned() {
            self.settings.active_crosshair = p.crosshair.clone();
            self.editing = p.crosshair;
            self.settings.active_preset_id = Some(p.id.clone());
            self.settings.recent_preset_ids.retain(|r| r != id);
            self.settings.recent_preset_ids.insert(0, id.to_string());
            self.settings.recent_preset_ids.truncate(8);
            self.dirty = true;
            self.push_overlay();
        }
    }
}
