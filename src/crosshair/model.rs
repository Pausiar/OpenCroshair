//! Core crosshair data model. Serializable, validated, versioned.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    /// Parse `#RRGGBB` or `#RRGGBBAA` (leading # optional).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        let p = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Self::rgb(p(0)?, p(2)?, p(4)?)),
            8 => Some(Self::new(p(0)?, p(2)?, p(4)?, p(6)?)),
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum Shape {
    #[default]
    Cross,
    Dot,
    Circle,
    CrossDot,
    /// Cross without outer lines: only center dot + circle optional via flags.
    CrossNoOuter,
    #[allow(dead_code)]
    TShape,
    /// Chevron / caret style.
    Chevron,
}

impl Shape {
    pub const ALL: [Shape; 7] = [
        Shape::Cross,
        Shape::Dot,
        Shape::Circle,
        Shape::CrossDot,
        Shape::CrossNoOuter,
        Shape::TShape,
        Shape::Chevron,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Shape::Cross => "Classic cross",
            Shape::Dot => "Center dot",
            Shape::Circle => "Circle",
            Shape::CrossDot => "Cross + dot",
            Shape::CrossNoOuter => "Dot ring",
            Shape::TShape => "T shape",
            Shape::Chevron => "Chevron",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crosshair {
    pub shape: Shape,
    // geometry (pixels, at 1x)
    pub length_h: f32,
    pub length_v: f32,
    pub thickness: f32,
    pub gap: f32,
    pub circle_radius: f32,
    pub dot_size: f32,
    /// Degrees, 0..360
    pub rotation: f32,
    /// Offset from the anchor point (center) in pixels.
    pub offset_x: f32,
    pub offset_y: f32,
    // element toggles
    pub show_lines: bool,
    pub show_dot: bool,
    pub show_circle: bool,
    // appearance
    pub color: Rgba,
    /// 0.0..=1.0 global opacity multiplier
    pub opacity: f32,
    pub outline: bool,
    pub outline_thickness: f32,
    pub outline_color: Rgba,
    pub shadow: bool,
    pub shadow_offset: f32,
    pub shadow_opacity: f32,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            shape: Shape::Cross,
            length_h: 10.0,
            length_v: 10.0,
            thickness: 2.0,
            gap: 4.0,
            circle_radius: 12.0,
            dot_size: 2.0,
            rotation: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            show_lines: true,
            show_dot: false,
            show_circle: false,
            color: Rgba::rgb(0, 209, 255),
            opacity: 1.0,
            outline: true,
            outline_thickness: 1.0,
            outline_color: Rgba::rgb(0, 0, 0),
            shadow: false,
            shadow_offset: 1.5,
            shadow_opacity: 0.5,
        }
    }
}

/// Maximum values used by validation and UI sliders.
pub mod limits {
    pub const MAX_LENGTH: f32 = 200.0;
    pub const MAX_THICKNESS: f32 = 40.0;
    pub const MAX_GAP: f32 = 100.0;
    pub const MAX_RADIUS: f32 = 200.0;
    pub const MAX_DOT: f32 = 60.0;
    pub const MAX_OFFSET: f32 = 2000.0;
    pub const MAX_OUTLINE: f32 = 10.0;
}

impl Crosshair {
    /// Pixel size of the square bitmap needed to hold the whole crosshair (always odd,
    /// includes outline/shadow margin; accounts for rotation and offset-free centered layout).
    pub fn bounding_size(&self) -> u32 {
        let reach = self
            .length_h
            .max(self.length_v)
            .mul_add(1.0, self.gap)
            .max(self.circle_radius + self.thickness)
            .max(self.dot_size)
            + self.thickness
            + self.outline_thickness * 2.0
            + if self.shadow { self.shadow_offset.abs() } else { 0.0 }
            + 4.0;
        let n = (reach * 2.0 * std::f32::consts::SQRT_2).ceil() as u32;
        n | 1
    }
}

/// A saved crosshair in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    /// Unix seconds.
    pub created_at: u64,
    pub favorite: bool,
    pub builtin: bool,
    pub crosshair: Crosshair,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Untitled".into(),
            description: None,
            category: "Custom".into(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            favorite: false,
            builtin: false,
            crosshair: Crosshair::default(),
        }
    }
}
