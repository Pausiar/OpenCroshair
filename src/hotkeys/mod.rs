//! Global hotkeys (agent). Contract:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleCrosshair,
    ToggleOverlay,
    NextPreset,
    PrevPreset,
    ToggleAutoProfiles,
}
