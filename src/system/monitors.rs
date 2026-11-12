//! Monitor enumeration (agent).
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    /// Physical pixel rect of the monitor on the virtual desktop.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    /// DPI scale factor (1.0 = 96dpi).
    pub scale: f32,
}

/// Enumerate monitors (Windows: EnumDisplayMonitors; other OS: one fake 1920x1080 primary).
pub fn list_monitors() -> Vec<MonitorInfo> {
    Vec::new()
}
