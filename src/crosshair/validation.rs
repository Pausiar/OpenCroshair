//! Validation and clamping of `Crosshair` values against `model::limits`.
use super::model::{limits, Crosshair};

/// Shadows are drawn down-right only, so the offset is 0..=MAX_SHADOW_OFFSET.
pub const MAX_SHADOW_OFFSET: f32 = limits::MAX_OUTLINE;
/// Rotation in degrees.
pub const MAX_ROTATION: f32 = 360.0;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min: f32,
    max: f32,
}

const LENGTH: Bounds = Bounds { min: 0.0, max: limits::MAX_LENGTH };
const THICKNESS: Bounds = Bounds { min: 0.0, max: limits::MAX_THICKNESS };
const GAP: Bounds = Bounds { min: 0.0, max: limits::MAX_GAP };
const RADIUS: Bounds = Bounds { min: 0.0, max: limits::MAX_RADIUS };
const DOT: Bounds = Bounds { min: 0.0, max: limits::MAX_DOT };
const ROTATION: Bounds = Bounds { min: 0.0, max: MAX_ROTATION };
const OFFSET: Bounds = Bounds { min: -limits::MAX_OFFSET, max: limits::MAX_OFFSET };
const UNIT: Bounds = Bounds { min: 0.0, max: 1.0 };
const OUTLINE: Bounds = Bounds { min: 0.0, max: limits::MAX_OUTLINE };
const SHADOW_OFFSET: Bounds = Bounds { min: 0.0, max: MAX_SHADOW_OFFSET };

/// One numeric field of a crosshair with its allowed range and clamp fallback.
struct FloatField<'a> {
    name: &'static str,
    value: &'a mut f32,
    bounds: Bounds,
    default: f32,
}

/// Every f32 field of `c`. Colors (u8), toggles and the shape enum are always valid.
fn float_fields<'a>(c: &'a mut Crosshair, d: &Crosshair) -> [FloatField<'a>; 13] {
    [
        FloatField { name: "length_h", value: &mut c.length_h, bounds: LENGTH, default: d.length_h },
        FloatField { name: "length_v", value: &mut c.length_v, bounds: LENGTH, default: d.length_v },
        FloatField { name: "thickness", value: &mut c.thickness, bounds: THICKNESS, default: d.thickness },
        FloatField { name: "gap", value: &mut c.gap, bounds: GAP, default: d.gap },
        FloatField { name: "circle_radius", value: &mut c.circle_radius, bounds: RADIUS, default: d.circle_radius },
        FloatField { name: "dot_size", value: &mut c.dot_size, bounds: DOT, default: d.dot_size },
        FloatField { name: "rotation", value: &mut c.rotation, bounds: ROTATION, default: d.rotation },
        FloatField { name: "offset_x", value: &mut c.offset_x, bounds: OFFSET, default: d.offset_x },
        FloatField { name: "offset_y", value: &mut c.offset_y, bounds: OFFSET, default: d.offset_y },
        FloatField { name: "opacity", value: &mut c.opacity, bounds: UNIT, default: d.opacity },
        FloatField { name: "outline_thickness", value: &mut c.outline_thickness, bounds: OUTLINE, default: d.outline_thickness },
        FloatField { name: "shadow_offset", value: &mut c.shadow_offset, bounds: SHADOW_OFFSET, default: d.shadow_offset },
        FloatField { name: "shadow_opacity", value: &mut c.shadow_opacity, bounds: UNIT, default: d.shadow_opacity },
    ]
}

/// Returns every out-of-range / NaN / infinite field (all fields are checked, not just the first).
pub fn validate(c: &Crosshair) -> Result<(), Vec<ValidationError>> {
    let mut copy = c.clone();
    let mut errors = Vec::new();
    for f in float_fields(&mut copy, &Crosshair::default()) {
        let v = *f.value;
        if !v.is_finite() {
            errors.push(ValidationError { field: f.name, message: "must be a finite number".into() });
        } else if v < f.bounds.min || v > f.bounds.max {
            errors.push(ValidationError {
                field: f.name,
                message: format!("must be between {} and {} (got {})", f.bounds.min, f.bounds.max, v),
            });
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Clamp all fields into valid range. NaN is replaced by the field's default; +/-inf clamps to the bound.
pub fn clamp(c: &mut Crosshair) {
    for f in float_fields(c, &Crosshair::default()) {
        let v = *f.value;
        *f.value = if v.is_nan() { f.default } else { v.clamp(f.bounds.min, f.bounds.max) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::Rgba;

    const NAMES: [&str; 13] = [
        "length_h",
        "length_v",
        "thickness",
        "gap",
        "circle_radius",
        "dot_size",
        "rotation",
        "offset_x",
        "offset_y",
        "opacity",
        "outline_thickness",
        "shadow_offset",
        "shadow_opacity",
    ];

    fn set_field(c: &mut Crosshair, name: &str, v: f32) {
        match name {
            "length_h" => c.length_h = v,
            "length_v" => c.length_v = v,
            "thickness" => c.thickness = v,
            "gap" => c.gap = v,
            "circle_radius" => c.circle_radius = v,
            "dot_size" => c.dot_size = v,
            "rotation" => c.rotation = v,
            "offset_x" => c.offset_x = v,
            "offset_y" => c.offset_y = v,
            "opacity" => c.opacity = v,
            "outline_thickness" => c.outline_thickness = v,
            "shadow_offset" => c.shadow_offset = v,
            "shadow_opacity" => c.shadow_opacity = v,
            other => panic!("unknown field {other}"),
        }
    }

    fn get_field(c: &Crosshair, name: &str) -> f32 {
        match name {
            "length_h" => c.length_h,
            "length_v" => c.length_v,
            "thickness" => c.thickness,
            "gap" => c.gap,
            "circle_radius" => c.circle_radius,
            "dot_size" => c.dot_size,
            "rotation" => c.rotation,
            "offset_x" => c.offset_x,
            "offset_y" => c.offset_y,
            "opacity" => c.opacity,
            "outline_thickness" => c.outline_thickness,
            "shadow_offset" => c.shadow_offset,
            "shadow_opacity" => c.shadow_opacity,
            other => panic!("unknown field {other}"),
        }
    }

    #[test]
    fn default_is_valid() {
        assert!(validate(&Crosshair::default()).is_ok());
    }

    #[test]
    fn nan_and_infinity_are_reported_for_every_field() {
        for name in NAMES {
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut c = Crosshair::default();
                set_field(&mut c, name, bad);
                let errs = validate(&c).expect_err("non-finite value must be rejected");
                assert!(errs.iter().any(|e| e.field == name), "{name} not reported for {bad}");
            }
        }
    }

    #[test]
    fn out_of_range_is_reported_for_every_field() {
        for name in NAMES {
            for bad in [-1000.0_f32, 1000.0] {
                let mut c = Crosshair::default();
                set_field(&mut c, name, bad);
                let errs = validate(&c).expect_err("out of range must be rejected");
                assert!(errs.iter().any(|e| e.field == name), "{name} not reported for {bad}");
            }
        }
    }

    #[test]
    fn opacity_and_rotation_ranges() {
        let mut c = Crosshair { opacity: 1.5, ..Crosshair::default() };
        assert!(validate(&c).unwrap_err().iter().any(|e| e.field == "opacity"));
        c.opacity = -0.1;
        assert!(validate(&c).unwrap_err().iter().any(|e| e.field == "opacity"));
        c.opacity = 0.0;
        assert!(validate(&c).is_ok());

        let mut c = Crosshair { rotation: 400.0, ..Crosshair::default() };
        assert!(validate(&c).unwrap_err().iter().any(|e| e.field == "rotation"));
        c.rotation = 360.0;
        assert!(validate(&c).is_ok());
    }

    #[test]
    fn validate_reports_all_errors_at_once() {
        let c = Crosshair { length_h: f32::NAN, gap: 500.0, opacity: 2.0, ..Crosshair::default() };
        let errs = validate(&c).unwrap_err();
        assert_eq!(errs.len(), 3);
        assert!(errs.iter().all(|e| !e.message.is_empty()));
    }

    #[test]
    fn clamp_fixes_every_field() {
        let mut c = Crosshair::default();
        for name in NAMES {
            set_field(&mut c, name, f32::INFINITY);
        }
        clamp(&mut c);
        assert!(validate(&c).is_ok(), "{:?}", validate(&c));

        for name in NAMES {
            set_field(&mut c, name, f32::NEG_INFINITY);
        }
        clamp(&mut c);
        assert!(validate(&c).is_ok(), "{:?}", validate(&c));
    }

    #[test]
    fn clamp_replaces_nan_with_default_and_bounds_inf() {
        let d = Crosshair::default();
        let mut c = Crosshair::default();
        for name in NAMES {
            set_field(&mut c, name, f32::NAN);
        }
        clamp(&mut c);
        for name in NAMES {
            assert_eq!(get_field(&c, name), get_field(&d, name), "{name}");
        }

        let mut c = Crosshair { length_h: f32::INFINITY, offset_x: f32::NEG_INFINITY, ..Crosshair::default() };
        clamp(&mut c);
        assert_eq!(c.length_h, limits::MAX_LENGTH);
        assert_eq!(c.offset_x, -limits::MAX_OFFSET);
    }

    #[test]
    fn clamp_leaves_valid_values_untouched_and_is_idempotent() {
        let original = Crosshair {
            length_h: 37.5,
            opacity: 0.25,
            rotation: 123.0,
            color: Rgba::new(1, 2, 3, 4),
            shadow: true,
            ..Crosshair::default()
        };
        let mut c = original.clone();
        clamp(&mut c);
        assert_eq!(c, original);
        clamp(&mut c);
        assert_eq!(c, original);
    }
}
