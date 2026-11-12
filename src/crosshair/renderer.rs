//! CPU rasterizer: Crosshair -> premultiplied RGBA bitmap. Shared by preview and overlay.
use super::model::Crosshair;

#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA8, row-major, top-down. len = width*height*4.
    pub pixels: Vec<u8>,
}

/// Render centered in a square bitmap of `bounding_size()*scale` px (offset NOT applied).
pub fn render(_c: &Crosshair, _scale: f32) -> Bitmap {
    Bitmap { width: 1, height: 1, pixels: vec![0; 4] }
}
