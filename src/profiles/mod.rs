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
        Self { id: uuid::Uuid::new_v4().to_string(), name: String::new(), exe: String::new(), preset_id: None, enabled: true }
    }
}
