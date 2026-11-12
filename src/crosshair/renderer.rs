//! CPU rasterizer: Crosshair -> premultiplied RGBA bitmap. Shared by preview and overlay.
//!
//! Each primitive is a signed distance function in its local, unrotated space. Coverage is
//! `clamp(0.5 - d, 0, 1)`, a one-pixel analytic anti-aliasing. Each pass (shadow, outline, fill)
//! takes the minimum distance over all of its primitives in a scratch buffer, so overlapping
//! primitives do not double-blend along their edges.
use super::model::{Crosshair, Rgba, Shape};
use super::validation;

#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA8, row-major, top-down. len = width*height*4.
    pub pixels: Vec<u8>,
}

/// Largest bitmap side in pixels (odd). `render` lowers the effective scale to respect it.
pub const MAX_BITMAP_SIDE: u32 = 2047;

/// Upper bound on primitives per crosshair: chevron = 8 segments, plus dot and ring.
const MAX_PRIMS: usize = 10;
const INV_255: f32 = 1.0 / 255.0;

/// Primitive in local space: origin at the crosshair center, y pointing down, no rotation.
#[derive(Debug, Clone, Copy)]
enum Prim {
    /// Axis-aligned box with center (cx, cy) and half extents (hx, hy).
    Rect { cx: f32, cy: f32, hx: f32, hy: f32 },
    /// Filled disc of radius r.
    Disc { r: f32 },
    /// Circle stroke of radius r and half stroke width `half`.
    Ring { r: f32, half: f32 },
    /// Capsule from (ax, ay) to (bx, by) with radius `half`.
    Segment {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        half: f32,
    },
}

impl Prim {
    /// Signed distance from a local point to the primitive (negative inside).
    fn sdf(self, x: f32, y: f32) -> f32 {
        match self {
            Prim::Rect { cx, cy, hx, hy } => {
                let qx = (x - cx).abs() - hx;
                let qy = (y - cy).abs() - hy;
                let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
                outside + qx.max(qy).min(0.0)
            }
            Prim::Disc { r } => (x * x + y * y).sqrt() - r,
            Prim::Ring { r, half } => ((x * x + y * y).sqrt() - r).abs() - half,
            Prim::Segment { ax, ay, bx, by, half } => {
                let (ex, ey) = (bx - ax, by - ay);
                let (px, py) = (x - ax, y - ay);
                let len2 = ex * ex + ey * ey;
                let t = if len2 > 0.0 {
                    ((px * ex + py * ey) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let dx = px - ex * t;
                let dy = py - ey * t;
                (dx * dx + dy * dy).sqrt() - half
            }
        }
    }

    /// Local-space bounds `[x0, y0, x1, y1]` before rotation.
    fn local_bounds(self) -> [f32; 4] {
        match self {
            Prim::Rect { cx, cy, hx, hy } => [cx - hx, cy - hy, cx + hx, cy + hy],
            Prim::Disc { r } => [-r, -r, r, r],
            Prim::Ring { r, half } => {
                let e = r + half;
                [-e, -e, e, e]
            }
            Prim::Segment { ax, ay, bx, by, half } => [
                ax.min(bx) - half,
                ay.min(by) - half,
                ax.max(bx) + half,
                ay.max(by) + half,
            ],
        }
    }
}

const FILLER: Prim = Prim::Disc { r: 0.0 };

struct PrimList {
    items: [Prim; MAX_PRIMS],
    len: usize,
}

impl PrimList {
    fn new() -> Self {
        Self {
            items: [FILLER; MAX_PRIMS],
            len: 0,
        }
    }
    fn push(&mut self, p: Prim) {
        if self.len < MAX_PRIMS {
            self.items[self.len] = p;
            self.len += 1;
        }
    }
    fn as_slice(&self) -> &[Prim] {
        &self.items[..self.len]
    }
}

/// Builds the local-space primitives for the crosshair, with all lengths multiplied by `k`.
///
/// The shape supplies its essential elements and the toggles add or remove optional ones:
/// - arms: `show_lines`, on the line shapes (Cross, CrossDot, TShape, Chevron)
/// - center dot: always for Dot, CrossDot and CrossNoOuter; otherwise `show_dot`
/// - ring: always for Circle and CrossNoOuter; otherwise `show_circle`
fn build_prims(c: &Crosshair, k: f32) -> PrimList {
    let mut list = PrimList::new();
    let shape = c.shape;
    let arms = c.show_lines && matches!(shape, Shape::Cross | Shape::CrossDot | Shape::TShape | Shape::Chevron);
    let dot = c.show_dot || matches!(shape, Shape::Dot | Shape::CrossDot | Shape::CrossNoOuter);
    let ring = c.show_circle || matches!(shape, Shape::Circle | Shape::CrossNoOuter);

    let half = c.thickness * k * 0.5;
    let g = c.gap * k;
    let lh = c.length_h * k;
    let lv = c.length_v * k;

    if arms {
        if shape == Shape::Chevron {
            // Four V-shaped arms, one per side. Each vertex sits `gap` from the center and the
            // arms reach `length_h` sideways and `length_v` outwards.
            let seg = |ax: f32, ay: f32, bx: f32, by: f32| Prim::Segment { ax, ay, bx, by, half };
            list.push(seg(0.0, -g, -lh, -(g + lv)));
            list.push(seg(0.0, -g, lh, -(g + lv)));
            list.push(seg(g, 0.0, g + lv, -lh));
            list.push(seg(g, 0.0, g + lv, lh));
            list.push(seg(0.0, g, lh, g + lv));
            list.push(seg(0.0, g, -lh, g + lv));
            list.push(seg(-g, 0.0, -(g + lv), lh));
            list.push(seg(-g, 0.0, -(g + lv), -lh));
        } else {
            if lh > 0.0 {
                list.push(Prim::Rect {
                    cx: g + lh * 0.5,
                    cy: 0.0,
                    hx: lh * 0.5,
                    hy: half,
                });
                list.push(Prim::Rect {
                    cx: -(g + lh * 0.5),
                    cy: 0.0,
                    hx: lh * 0.5,
                    hy: half,
                });
            }
            if lv > 0.0 {
                list.push(Prim::Rect {
                    cx: 0.0,
                    cy: g + lv * 0.5,
                    hx: half,
                    hy: lv * 0.5,
                });
                if shape != Shape::TShape {
                    list.push(Prim::Rect {
                        cx: 0.0,
                        cy: -(g + lv * 0.5),
                        hx: half,
                        hy: lv * 0.5,
                    });
                }
            }
        }
    }
    if dot && c.dot_size > 0.0 {
        list.push(Prim::Disc {
            r: c.dot_size * k * 0.5,
        });
    }
    if ring && c.circle_radius > 0.0 {
        list.push(Prim::Ring {
            r: c.circle_radius * k,
            half,
        });
    }
    list
}

/// One compositing pass over all primitives.
struct Pass {
    /// Screen-space translation of the whole shape (used by the shadow).
    dx: f32,
    dy: f32,
    /// Dilation in pixels (used by the outline).
    grow: f32,
    color: Rgba,
    /// Multiplier on top of `color.a`, includes the global opacity.
    alpha: f32,
}

/// Composites one pass onto the premultiplied RGBA buffer (source-over).
///
/// Works per primitive inside its own pixel box, storing the minimum signed distance in a
/// scratch buffer covering the union of those boxes, then blends the covered pixels once.
fn draw_pass(buf: &mut [u8], side: u32, prims: &[Prim], rot: (f32, f32), pass: &Pass, scratch: &mut Vec<f32>) {
    let (cos, sin) = rot;
    let side_i = side as i32;
    let center = side as f32 * 0.5;
    let margin = pass.grow.max(0.0) + 1.0;

    // Per-primitive pixel box (half-open), and the union box.
    let mut boxes = [(0i32, 0i32, 0i32, 0i32); MAX_PRIMS];
    let (mut ux0, mut uy0, mut ux1, mut uy1) = (side_i, side_i, 0i32, 0i32);
    for (i, p) in prims.iter().enumerate() {
        let [x0, y0, x1, y1] = p.local_bounds();
        let (x0, y0, x1, y1) = (x0 - margin, y0 - margin, x1 + margin, y1 + margin);
        let (mut mnx, mut mny) = (f32::INFINITY, f32::INFINITY);
        let (mut mxx, mut mxy) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
            let sx = cos * x - sin * y + center + pass.dx;
            let sy = sin * x + cos * y + center + pass.dy;
            mnx = mnx.min(sx);
            mny = mny.min(sy);
            mxx = mxx.max(sx);
            mxy = mxy.max(sy);
        }
        let bx0 = (mnx.floor() as i32).clamp(0, side_i);
        let by0 = (mny.floor() as i32).clamp(0, side_i);
        let bx1 = (mxx.ceil() as i32).clamp(0, side_i);
        let by1 = (mxy.ceil() as i32).clamp(0, side_i);
        boxes[i] = (bx0, by0, bx1, by1);
        if bx0 < bx1 && by0 < by1 {
            ux0 = ux0.min(bx0);
            uy0 = uy0.min(by0);
            ux1 = ux1.max(bx1);
            uy1 = uy1.max(by1);
        }
    }
    if ux0 >= ux1 || uy0 >= uy1 {
        return;
    }

    let uw = (ux1 - ux0) as usize;
    scratch.clear();
    scratch.resize(uw * (uy1 - uy0) as usize, f32::INFINITY);

    for (i, p) in prims.iter().enumerate() {
        let (bx0, by0, bx1, by1) = boxes[i];
        for y in by0..by1 {
            let qy = y as f32 + 0.5 - center - pass.dy;
            let row = (y - uy0) as usize * uw;
            for x in bx0..bx1 {
                let qx = x as f32 + 0.5 - center - pass.dx;
                // Inverse rotation: local = R(-theta) * q.
                let lx = cos * qx + sin * qy;
                let ly = -sin * qx + cos * qy;
                let d = p.sdf(lx, ly) - pass.grow;
                let cell = &mut scratch[row + (x - ux0) as usize];
                if d < *cell {
                    *cell = d;
                }
            }
        }
    }

    let src = [
        pass.color.r as f32 * INV_255,
        pass.color.g as f32 * INV_255,
        pass.color.b as f32 * INV_255,
    ];
    let base_alpha = pass.color.a as f32 * INV_255 * pass.alpha;
    let stride = side as usize * 4;
    for y in uy0..uy1 {
        let row = (y - uy0) as usize * uw;
        for x in ux0..ux1 {
            let d = scratch[row + (x - ux0) as usize];
            let cov = (0.5 - d).clamp(0.0, 1.0);
            let a = base_alpha * cov;
            if a.is_nan() || a <= 0.0 {
                continue;
            }
            let i = y as usize * stride + x as usize * 4;
            let da = buf[i + 3] as f32 * INV_255;
            let out_a = (a + da * (1.0 - a)) * 255.0;
            let out_a8 = out_a.round().clamp(0.0, 255.0);
            for ch in 0..3 {
                let dst = buf[i + ch] as f32 * INV_255;
                let out = (src[ch] * a + dst * (1.0 - a)) * 255.0;
                buf[i + ch] = out.round().clamp(0.0, out_a8) as u8;
            }
            buf[i + 3] = out_a8 as u8;
        }
    }
}

/// Render centered in a square bitmap of `bounding_size()*scale` px (offset NOT applied).
///
/// The crosshair is sanitized with `validation::clamp` first, so NaN and infinite values never
/// reach the rasterizer. A non-finite or non-positive `scale` is treated as 1. The scale is
/// lowered if the bitmap would exceed `MAX_BITMAP_SIDE`. The side is always odd.
/// Draw order: shadow, outline, fill.
pub fn render(c: &Crosshair, scale: f32) -> Bitmap {
    let mut c = c.clone();
    validation::clamp(&mut c);

    let base = c.bounding_size().max(1);
    let mut s = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    s = s.min(MAX_BITMAP_SIDE as f32 / base as f32);
    let mut side = ((base as f32 * s).ceil() as u32).clamp(1, MAX_BITMAP_SIDE);
    if side.is_multiple_of(2) {
        side += 1;
    }

    let list = build_prims(&c, s);
    let prims = list.as_slice();
    let radians = c.rotation.to_radians();
    let rot = (radians.cos(), radians.sin());
    let opacity = c.opacity;

    let mut pixels = vec![0u8; side as usize * side as usize * 4];
    let mut scratch = Vec::new();

    if c.shadow {
        let off = c.shadow_offset * s;
        let pass = Pass {
            dx: off,
            dy: off,
            grow: 0.0,
            color: Rgba::rgb(0, 0, 0),
            alpha: opacity * c.shadow_opacity,
        };
        draw_pass(&mut pixels, side, prims, rot, &pass, &mut scratch);
    }
    if c.outline && c.outline_thickness > 0.0 {
        let pass = Pass {
            dx: 0.0,
            dy: 0.0,
            grow: c.outline_thickness * s,
            color: c.outline_color,
            alpha: opacity,
        };
        draw_pass(&mut pixels, side, prims, rot, &pass, &mut scratch);
    }
    let fill = Pass {
        dx: 0.0,
        dy: 0.0,
        grow: 0.0,
        color: c.color,
        alpha: opacity,
    };
    draw_pass(&mut pixels, side, prims, rot, &fill, &mut scratch);

    Bitmap {
        width: side,
        height: side,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::Shape;

    fn alpha(b: &Bitmap, x: u32, y: u32) -> u8 {
        b.pixels[((y * b.width + x) * 4 + 3) as usize]
    }

    fn px(b: &Bitmap, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * b.width + x) * 4) as usize;
        [b.pixels[i], b.pixels[i + 1], b.pixels[i + 2], b.pixels[i + 3]]
    }

    /// Crosshair with no outline and no shadow, so only the fill is drawn.
    fn plain(shape: Shape) -> Crosshair {
        Crosshair {
            shape,
            outline: false,
            shadow: false,
            ..Crosshair::default()
        }
    }

    #[test]
    fn bitmap_is_odd_square_and_matches_bounding_size() {
        let c = Crosshair::default();
        for scale in [0.5_f32, 1.0, 1.5, 2.0, 3.7] {
            let b = render(&c, scale);
            assert_eq!(b.width, b.height);
            assert_eq!(b.width % 2, 1, "side must be odd");
            assert_eq!(b.pixels.len(), (b.width * b.height * 4) as usize);
            let expected = ((c.bounding_size() as f32 * scale).ceil() as u32) | 1;
            assert_eq!(b.width, expected, "scale {scale}");
        }
    }

    #[test]
    fn center_pixel_is_opaque_and_mirror_symmetric() {
        let c = Crosshair {
            show_dot: true,
            dot_size: 4.0,
            ..Crosshair::default()
        };
        let b = render(&c, 1.0);
        let m = (b.width - 1) / 2;
        assert_eq!(alpha(&b, m, m), 255);
        for y in 0..b.height {
            for x in 0..b.width {
                let a = alpha(&b, x, y);
                assert!(
                    a.abs_diff(alpha(&b, b.width - 1 - x, y)) <= 1,
                    "horizontal mirror at {x},{y}"
                );
                assert!(
                    a.abs_diff(alpha(&b, x, b.height - 1 - y)) <= 1,
                    "vertical mirror at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn corners_are_transparent() {
        for scale in [1.0_f32, 2.5] {
            let b = render(&Crosshair::default(), scale);
            let last = b.width - 1;
            for (x, y) in [(0, 0), (last, 0), (0, last), (last, last)] {
                assert_eq!(alpha(&b, x, y), 0, "corner {x},{y} at scale {scale}");
            }
        }
    }

    #[test]
    fn rotation_90_swaps_horizontal_and_vertical_lengths() {
        let rotated = Crosshair {
            length_h: 10.0,
            length_v: 4.0,
            rotation: 90.0,
            ..plain(Shape::Cross)
        };
        let swapped = Crosshair {
            length_h: 4.0,
            length_v: 10.0,
            rotation: 0.0,
            ..plain(Shape::Cross)
        };
        let unrotated = Crosshair {
            length_h: 10.0,
            length_v: 4.0,
            rotation: 0.0,
            ..plain(Shape::Cross)
        };

        let a = render(&rotated, 1.0);
        let b = render(&swapped, 1.0);
        let u = render(&unrotated, 1.0);
        assert_eq!(a.width, b.width);
        let max_diff = a
            .pixels
            .iter()
            .zip(&b.pixels)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        assert!(max_diff <= 2, "rotated 90 differs from swapped lengths by {max_diff}");
        assert_ne!(a.pixels, u.pixels, "rotation must change the image");
    }

    #[test]
    fn outline_is_drawn_behind_fill() {
        let c = Crosshair {
            color: Rgba::rgb(0, 255, 0),
            outline: true,
            outline_thickness: 3.0,
            outline_color: Rgba::rgb(255, 0, 0),
            ..plain(Shape::Cross)
        };
        let b = render(&c, 1.0);
        let has_outline = b.pixels.chunks_exact(4).any(|p| p[3] > 200 && p[0] > 200 && p[1] == 0);
        let has_fill = b.pixels.chunks_exact(4).any(|p| p[3] > 200 && p[1] > 200 && p[0] == 0);
        assert!(has_outline, "outline pixels missing");
        assert!(has_fill, "fill pixels missing");

        let without = render(&Crosshair { outline: false, ..c }, 1.0);
        assert!(
            without.pixels.chunks_exact(4).all(|p| p[0] == 0),
            "outline drawn while disabled"
        );
    }

    #[test]
    fn shadow_is_black_and_offset_down_right() {
        let c = Crosshair {
            shadow: true,
            shadow_offset: 4.0,
            shadow_opacity: 1.0,
            ..plain(Shape::Dot)
        };
        let c = Crosshair {
            dot_size: 6.0,
            color: Rgba::rgb(255, 255, 255),
            ..c
        };
        let b = render(&c, 1.0);
        let m = (b.width - 1) / 2;
        // Shadow disc centred at (4, 4) with radius 3: pixel offset (6, 4) is inside it, outside the fill.
        assert_eq!(px(&b, m + 6, m + 4), [0, 0, 0, 255]);
        // Up-left of the shape there is no shadow.
        assert_eq!(alpha(&b, m - 5, m - 5), 0);

        // Without the shadow flag no black pixels are produced.
        let no_shadow = render(&Crosshair { shadow: false, ..c }, 1.0);
        assert!(
            no_shadow
                .pixels
                .chunks_exact(4)
                .all(|p| p[3] == 0 || p[0] > 0 || p[1] > 0 || p[2] > 0)
        );
    }

    #[test]
    fn offset_is_not_applied() {
        let a = render(&plain(Shape::Cross), 1.0);
        let b = render(
            &Crosshair {
                offset_x: 50.0,
                offset_y: -30.0,
                ..plain(Shape::Cross)
            },
            1.0,
        );
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn opacity_scales_alpha() {
        let c = Crosshair {
            show_dot: true,
            dot_size: 4.0,
            opacity: 0.5,
            ..plain(Shape::Cross)
        };
        let b = render(&c, 1.0);
        let m = (b.width - 1) / 2;
        let a = alpha(&b, m, m);
        assert!((120..=135).contains(&a), "alpha {a}");
    }

    #[test]
    fn tshape_has_no_top_arm() {
        let c = Crosshair {
            length_h: 10.0,
            length_v: 10.0,
            gap: 4.0,
            thickness: 2.0,
            ..plain(Shape::TShape)
        };
        let b = render(&c, 1.0);
        let m = (b.width - 1) / 2;
        assert_eq!(alpha(&b, m, m - 8), 0, "top arm must be absent");
        assert!(alpha(&b, m, m + 8) > 200, "bottom arm must be present");
    }

    #[test]
    fn every_shape_draws_something() {
        for shape in Shape::ALL {
            let b = render(
                &Crosshair {
                    shape,
                    ..Crosshair::default()
                },
                1.0,
            );
            assert!(b.pixels.chunks_exact(4).any(|p| p[3] > 0), "{shape:?} drew nothing");
        }
    }

    #[test]
    fn scale_is_capped_to_max_bitmap_side() {
        let b = render(&Crosshair::default(), 1e9);
        assert_eq!(b.width, MAX_BITMAP_SIDE);
        assert_eq!(b.width % 2, 1);
    }

    #[test]
    fn extreme_values_do_not_panic() {
        let extremes = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e9,
            1e9,
            0.0,
            -0.0,
            f32::MIN_POSITIVE,
        ];
        let setters: [fn(&mut Crosshair, f32); 13] = [
            |c, v| c.length_h = v,
            |c, v| c.length_v = v,
            |c, v| c.thickness = v,
            |c, v| c.gap = v,
            |c, v| c.circle_radius = v,
            |c, v| c.dot_size = v,
            |c, v| c.rotation = v,
            |c, v| c.offset_x = v,
            |c, v| c.offset_y = v,
            |c, v| c.opacity = v,
            |c, v| c.outline_thickness = v,
            |c, v| c.shadow_offset = v,
            |c, v| c.shadow_opacity = v,
        ];
        for shape in Shape::ALL {
            for set in setters {
                for v in extremes {
                    let mut c = Crosshair {
                        shape,
                        show_dot: true,
                        show_circle: true,
                        outline: true,
                        shadow: true,
                        ..Crosshair::default()
                    };
                    set(&mut c, v);
                    let b = render(&c, 1.0);
                    assert_eq!(b.pixels.len(), (b.width * b.height * 4) as usize);
                }
            }
        }

        let all_nan = Crosshair {
            length_h: f32::NAN,
            length_v: f32::NAN,
            thickness: f32::NAN,
            gap: f32::NAN,
            circle_radius: f32::NAN,
            dot_size: f32::NAN,
            rotation: f32::NAN,
            offset_x: f32::NAN,
            offset_y: f32::NAN,
            opacity: f32::NAN,
            outline_thickness: f32::NAN,
            shadow_offset: f32::NAN,
            shadow_opacity: f32::NAN,
            shadow: true,
            outline: true,
            ..Crosshair::default()
        };
        for scale in [f32::NAN, 0.0, -1.0, f32::INFINITY, 1e9, 1e-9] {
            let b = render(&all_nan, scale);
            assert_eq!(b.pixels.len(), (b.width * b.height * 4) as usize);
            assert!(b.width >= 1 && b.width <= MAX_BITMAP_SIDE && b.width % 2 == 1);
        }
    }
}
