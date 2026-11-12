use serde::{Deserialize, Serialize};

pub mod detection;
pub mod manager;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameProfile {
    pub id: String,
    pub name: String,
    /// Executable file name, matched case-insensitively (e.g. "cs2.exe").
    pub exe: String,
    /// Preset applied while this game is in the foreground.
    pub preset_id: Option<String>,
    pub enabled: bool,
}

impl Default for GameProfile {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            exe: String::new(),
            preset_id: None,
            enabled: true,
        }
    }
}

impl GameProfile {
    /// New enabled profile with a fresh id. `exe` is normalized to a lowercase file name
    /// (a full path is accepted and reduced to its file name).
    #[allow(dead_code)]
    pub fn new(name: &str, exe: &str, preset_id: Option<String>) -> Self {
        Self {
            name: name.trim().to_string(),
            exe: manager::normalize_exe(exe),
            preset_id,
            ..Self::default()
        }
    }

    /// True when this profile is enabled and `exe` (file name or path, any case) names its executable.
    pub fn matches_exe(&self, exe: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let mine = manager::normalize_exe(&self.exe);
        !mine.is_empty() && mine == manager::normalize_exe(exe)
    }
}
