//! Settings + on-disk layout. Implemented by the storage agent; types are the shared contract.
use crate::crosshair::model::{Crosshair, Preset};
use crate::profiles::GameProfile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    /// Strings like "Ctrl+Alt+F1" (parsed by hotkeys module).
    pub toggle_crosshair: String,
    pub toggle_overlay: String,
    pub next_preset: String,
    pub prev_preset: String,
    pub toggle_auto_profiles: String,
}
impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            toggle_crosshair: "Ctrl+Alt+F1".into(),
            toggle_overlay: "Ctrl+Alt+F2".into(),
            next_preset: "Ctrl+Alt+Right".into(),
            prev_preset: "Ctrl+Alt+Left".into(),
            toggle_auto_profiles: "Ctrl+Alt+F3".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CenterMode {
    /// Center of the chosen monitor.
    #[default]
    Monitor,
    /// Center of the foreground game window (falls back to monitor).
    GameWindow,
    /// Absolute screen position (custom_x/custom_y).
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PositionSettings {
    pub mode: CenterMode,
    /// Monitor index as enumerated by system::monitors (None = primary).
    pub monitor: Option<usize>,
    pub offset_x: i32,
    pub offset_y: i32,
    pub custom_x: i32,
    pub custom_y: i32,
}
impl Default for PositionSettings {
    fn default() -> Self {
        Self { mode: CenterMode::Monitor, monitor: None, offset_x: 0, offset_y: 0, custom_x: 0, custom_y: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Crosshair currently applied (editor "Apply").
    pub active_crosshair: Crosshair,
    pub active_preset_id: Option<String>,
    pub crosshair_visible: bool,
    pub overlay_enabled: bool,
    pub position: PositionSettings,
    pub hotkeys: HotkeySettings,
    pub auto_profiles: bool,
    /// Poll interval (ms) for foreground detection, 250..=5000.
    pub detection_interval_ms: u32,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub launch_at_startup: bool,
    pub default_profile_preset_id: Option<String>,
    pub preview_dark_background: bool,
    pub preview_zoom: f32,
    pub recent_preset_ids: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            active_crosshair: Crosshair::default(),
            active_preset_id: None,
            crosshair_visible: true,
            overlay_enabled: false,
            position: PositionSettings::default(),
            hotkeys: HotkeySettings::default(),
            auto_profiles: false,
            detection_interval_ms: 1000,
            minimize_to_tray: true,
            start_minimized: false,
            launch_at_startup: false,
            default_profile_preset_id: None,
            preview_dark_background: true,
            preview_zoom: 4.0,
            recent_preset_ids: Vec::new(),
        }
    }
}

/// Resolved data directories (config, presets, profiles, logs).
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

/// Everything loaded from disk. Corrupt files are renamed `*.corrupt-<ts>` and defaults used.
#[derive(Debug, Default)]
pub struct LoadedData {
    pub settings: Settings,
    pub presets: Vec<Preset>,
    pub profiles: Vec<GameProfile>,
    /// Human readable warnings to show in UI (e.g. "presets.json was corrupt").
    pub warnings: Vec<String>,
}
