use super::model::{Crosshair, Preset, Rgba, Shape};

const WHITE: Rgba = Rgba::rgb(241, 245, 249);
const BLACK: Rgba = Rgba::rgb(0, 0, 0);

fn preset(slug: &str, name: &str, category: &str, description: &str, crosshair: Crosshair) -> Preset {
    Preset {
        id: format!("builtin-{slug}"),
        name: name.into(),
        description: Some(description.into()),
        category: category.into(),
        created_at: 0,
        favorite: false,
        builtin: true,
        crosshair,
    }
}

/// Original built-in presets (builtin = true, stable ids like "builtin-classic-green").
pub fn builtin_presets() -> Vec<Preset> {
    let minimal = "Minimal";
    let classic = "Classic";
    let dot = "Dot";
    let circle = "Circle";
    let precision = "Precision";
    let high = "High visibility";
    let dynamic = "Dynamic";
    let outline_only = "Outline only";
    let soft = "Soft";

    vec![
        preset(
            "minimal-hairline",
            "Hairline",
            minimal,
            "Thin one-pixel cross with a short gap.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 5.0,
                length_v: 5.0,
                thickness: 1.0,
                gap: 3.0,
                color: WHITE,
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "minimal-pip",
            "Pip",
            minimal,
            "Tiny cross with a center pip for quick aiming.",
            Crosshair {
                shape: Shape::CrossDot,
                length_h: 4.0,
                length_v: 4.0,
                thickness: 2.0,
                gap: 2.0,
                dot_size: 2.0,
                color: WHITE,
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "minimal-tee",
            "Minimal tee",
            minimal,
            "Bottom-stem T shape at slightly reduced opacity.",
            Crosshair {
                shape: Shape::TShape,
                length_h: 6.0,
                length_v: 6.0,
                thickness: 2.0,
                gap: 3.0,
                color: WHITE,
                opacity: 0.9,
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-green",
            "Classic green",
            classic,
            "Medium cross in bright green with a dark outline.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 10.0,
                length_v: 10.0,
                thickness: 2.0,
                gap: 4.0,
                color: Rgba::rgb(34, 255, 136),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-red-dot",
            "Red cross dot",
            classic,
            "Cross with a red center dot.",
            Crosshair {
                shape: Shape::CrossDot,
                length_h: 8.0,
                length_v: 8.0,
                thickness: 2.0,
                gap: 4.0,
                dot_size: 3.0,
                color: Rgba::rgb(255, 77, 77),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-cyan-shadow",
            "Cyan shadow",
            classic,
            "Wide cyan cross with a soft drop shadow.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 12.0,
                length_v: 12.0,
                thickness: 2.0,
                gap: 6.0,
                color: Rgba::rgb(34, 211, 238),
                outline: false,
                shadow: true,
                shadow_offset: 1.5,
                shadow_opacity: 0.5,
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-white",
            "White dot",
            dot,
            "Single centered dot with a dark outline.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 4.0,
                color: Rgba::rgb(255, 255, 255),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-amber",
            "Amber dot",
            dot,
            "Small amber dot with a drop shadow.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 3.0,
                color: Rgba::rgb(251, 191, 36),
                outline: false,
                shadow: true,
                shadow_offset: 1.0,
                shadow_opacity: 0.6,
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-ring-blue",
            "Blue dot ring",
            dot,
            "Center dot inside a thin blue ring.",
            Crosshair {
                shape: Shape::CrossNoOuter,
                dot_size: 3.0,
                circle_radius: 6.0,
                thickness: 1.5,
                color: Rgba::rgb(96, 165, 250),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "circle-ring",
            "Ring",
            circle,
            "Thin circle without a center dot.",
            Crosshair {
                shape: Shape::Circle,
                circle_radius: 8.0,
                thickness: 1.5,
                color: WHITE,
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "circle-lime-dot",
            "Lime ring dot",
            circle,
            "Wide lime ring with a small center dot.",
            Crosshair {
                shape: Shape::Circle,
                circle_radius: 14.0,
                thickness: 1.0,
                show_dot: true,
                dot_size: 2.0,
                color: Rgba::rgb(163, 230, 53),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-fine",
            "Fine cyan",
            precision,
            "Long hairline cross with a center dot for pixel-level aim.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 14.0,
                length_v: 14.0,
                thickness: 1.0,
                gap: 2.0,
                show_dot: true,
                dot_size: 1.0,
                color: Rgba::rgb(0, 229, 255),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-sniper",
            "Long sniper",
            precision,
            "Long thin cross with a wide gap and slightly reduced opacity.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 20.0,
                length_v: 20.0,
                thickness: 1.0,
                gap: 8.0,
                show_dot: true,
                dot_size: 2.0,
                color: Rgba::rgb(226, 232, 240),
                opacity: 0.85,
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-tee-pink",
            "Pink tee",
            precision,
            "Thin T shape for precise horizontal tracking.",
            Crosshair {
                shape: Shape::TShape,
                length_h: 12.0,
                length_v: 12.0,
                thickness: 1.0,
                gap: 5.0,
                color: Rgba::rgb(244, 114, 182),
                outline_thickness: 1.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "highvis-magenta",
            "Magenta bold",
            high,
            "Thick magenta cross with a heavy outline and shadow.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 16.0,
                length_v: 16.0,
                thickness: 4.0,
                gap: 6.0,
                color: Rgba::rgb(255, 0, 255),
                outline_thickness: 2.0,
                outline_color: BLACK,
                shadow: true,
                shadow_offset: 2.0,
                shadow_opacity: 0.6,
                ..Crosshair::default()
            },
        ),
        preset(
            "highvis-yellow-chevron",
            "Yellow chevron",
            high,
            "Bold chevrons pointing at the center.",
            Crosshair {
                shape: Shape::Chevron,
                length_h: 8.0,
                length_v: 10.0,
                thickness: 3.0,
                gap: 6.0,
                color: Rgba::rgb(255, 235, 59),
                outline_thickness: 2.0,
                outline_color: BLACK,
                ..Crosshair::default()
            },
        ),
        preset(
            "minimal-micro-dot",
            "Micro dot",
            minimal,
            "Smallest possible center dot with a dark outline.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 1.5,
                color: WHITE,
                ..Crosshair::default()
            },
        ),
        preset(
            "minimal-hair-plus",
            "Hair plus",
            minimal,
            "Very short one-pixel plus sign with a tight gap.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 3.0,
                length_v: 3.0,
                thickness: 1.0,
                gap: 1.0,
                color: WHITE,
                ..Crosshair::default()
            },
        ),
        preset(
            "minimal-quiet-cross",
            "Quiet cross",
            minimal,
            "Short thin cross that stays out of the way at 70 percent opacity.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 6.0,
                length_v: 6.0,
                thickness: 1.0,
                gap: 4.0,
                color: WHITE,
                opacity: 0.7,
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-white-bold",
            "White bold cross",
            classic,
            "Sturdy off-white cross with a wide stroke.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 8.0,
                length_v: 8.0,
                thickness: 3.0,
                gap: 3.0,
                color: WHITE,
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-teal",
            "Teal cross",
            classic,
            "Long teal cross with a generous center gap.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 10.0,
                length_v: 10.0,
                thickness: 2.0,
                gap: 5.0,
                color: Rgba::rgb(45, 212, 191),
                ..Crosshair::default()
            },
        ),
        preset(
            "classic-orange-dot",
            "Orange dot cross",
            classic,
            "Compact orange cross with a small center dot.",
            Crosshair {
                shape: Shape::CrossDot,
                length_h: 7.0,
                length_v: 7.0,
                thickness: 2.0,
                gap: 4.0,
                dot_size: 2.0,
                color: Rgba::rgb(251, 146, 60),
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-green-large",
            "Large green dot",
            dot,
            "Big bright green center dot that is easy to spot.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 5.0,
                color: Rgba::rgb(74, 222, 128),
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-red-micro",
            "Red micro dot",
            dot,
            "Tiny red dot with a thin outline.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 2.0,
                color: Rgba::rgb(244, 63, 94),
                outline: true,
                outline_thickness: 1.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "dot-violet-shadow",
            "Violet shadow dot",
            dot,
            "Violet center dot with a soft drop shadow.",
            Crosshair {
                shape: Shape::Dot,
                dot_size: 3.0,
                color: Rgba::rgb(167, 139, 250),
                shadow: true,
                ..Crosshair::default()
            },
        ),
        preset(
            "circle-wide-ring",
            "Wide ring",
            circle,
            "Large thin ring with no center mark.",
            Crosshair {
                shape: Shape::Circle,
                circle_radius: 24.0,
                thickness: 1.5,
                ..Crosshair::default()
            },
        ),
        preset(
            "circle-cyan-ring-dot",
            "Cyan ring dot",
            circle,
            "Small cyan ring around a center dot.",
            Crosshair {
                shape: Shape::CrossNoOuter,
                circle_radius: 10.0,
                dot_size: 2.0,
                show_circle: true,
                color: Rgba::rgb(34, 211, 238),
                ..Crosshair::default()
            },
        ),
        preset(
            "circle-ring-cross",
            "Ring cross",
            circle,
            "Short cross inside a wide ring with a clear gap.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 6.0,
                length_v: 6.0,
                gap: 8.0,
                show_circle: true,
                circle_radius: 14.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-x-cross",
            "X cross",
            precision,
            "Cross rotated 45 degrees into a fine X shape.",
            Crosshair {
                shape: Shape::Cross,
                rotation: 45.0,
                length_h: 6.0,
                length_v: 6.0,
                thickness: 1.0,
                gap: 3.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-long-hair",
            "Long hairline",
            precision,
            "Long, thin cross with a wide center gap at 85 percent opacity.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 30.0,
                length_v: 30.0,
                thickness: 1.0,
                gap: 8.0,
                opacity: 0.85,
                ..Crosshair::default()
            },
        ),
        preset(
            "precision-fine-tee",
            "Fine tee",
            precision,
            "Thin T shape with a short stem for horizontal alignment.",
            Crosshair {
                shape: Shape::TShape,
                length_h: 12.0,
                length_v: 12.0,
                thickness: 1.0,
                gap: 4.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "highvis-neon-chevron",
            "Neon chevron",
            high,
            "Bright green chevrons with a dark outline for busy backgrounds.",
            Crosshair {
                shape: Shape::Chevron,
                length_h: 10.0,
                length_v: 10.0,
                thickness: 3.0,
                gap: 4.0,
                color: Rgba::rgb(34, 255, 136),
                outline_thickness: 2.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "highvis-bold-x",
            "Bold X",
            high,
            "Thick yellow X with a heavy outline.",
            Crosshair {
                shape: Shape::Cross,
                rotation: 45.0,
                length_h: 10.0,
                length_v: 10.0,
                thickness: 4.0,
                gap: 4.0,
                color: Rgba::rgb(255, 235, 59),
                outline_thickness: 2.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "dynamic-wide-gap",
            "Wide gap",
            dynamic,
            "Short arms with a wide center gap for open, uncluttered aim.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 8.0,
                length_v: 8.0,
                thickness: 2.0,
                gap: 14.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "dynamic-tall-cross",
            "Tall cross",
            dynamic,
            "Vertically stretched cross with short horizontal arms.",
            Crosshair {
                shape: Shape::Cross,
                length_v: 16.0,
                length_h: 6.0,
                thickness: 2.0,
                gap: 4.0,
                ..Crosshair::default()
            },
        ),
        preset(
            "outline-ghost-cross",
            "Ghost cross",
            outline_only,
            "Solid black cross framed by a thin white outline, for light and dark scenes.",
            Crosshair {
                shape: Shape::Cross,
                length_h: 8.0,
                length_v: 8.0,
                thickness: 2.0,
                color: BLACK,
                opacity: 1.0,
                outline: true,
                outline_thickness: 1.5,
                outline_color: WHITE,
                ..Crosshair::default()
            },
        ),
        preset(
            "soft-blue",
            "Soft blue",
            soft,
            "Pale blue cross at 80 percent opacity with a faint shadow.",
            Crosshair {
                shape: Shape::Cross,
                color: Rgba::rgb(147, 197, 253),
                opacity: 0.8,
                shadow: true,
                shadow_opacity: 0.3,
                ..Crosshair::default()
            },
        ),
        preset(
            "soft-cream",
            "Soft cream",
            soft,
            "Warm cream cross with a small center dot.",
            Crosshair {
                shape: Shape::CrossDot,
                dot_size: 3.0,
                color: Rgba::rgb(255, 247, 230),
                ..Crosshair::default()
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::renderer;
    use crate::crosshair::validation::validate;
    use std::collections::HashSet;

    #[test]
    fn has_enough_presets_across_all_categories() {
        let presets = builtin_presets();
        assert!(presets.len() >= 36, "only {} presets", presets.len());
        let categories: HashSet<&str> = presets.iter().map(|p| p.category.as_str()).collect();
        let expected = [
            "Minimal",
            "Classic",
            "Dot",
            "Circle",
            "Precision",
            "High visibility",
            "Dynamic",
            "Outline only",
            "Soft",
        ];
        for name in expected {
            assert!(categories.contains(name), "missing category {name}");
        }
        assert_eq!(
            categories.len(),
            expected.len(),
            "unexpected categories: {categories:?}"
        );
    }

    #[test]
    fn every_preset_renders_visible_pixels() {
        for p in builtin_presets() {
            let bmp = renderer::render(&p.crosshair, 1.0);
            assert_eq!(bmp.pixels.len(), (bmp.width * bmp.height * 4) as usize, "{}", p.id);
            assert!(
                bmp.pixels.chunks_exact(4).any(|px| px[3] > 0),
                "{} rendered no pixels",
                p.id
            );
        }
    }

    #[test]
    fn ids_are_unique_and_prefixed() {
        let presets = builtin_presets();
        let ids: HashSet<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), presets.len());
        for p in &presets {
            assert!(p.id.starts_with("builtin-"), "{}", p.id);
            assert!(p.id.len() > "builtin-".len(), "{}", p.id);
        }
    }

    #[test]
    fn all_presets_validate() {
        for p in builtin_presets() {
            if let Err(errs) = validate(&p.crosshair) {
                panic!("preset {} is invalid: {:?}", p.id, errs);
            }
        }
    }

    #[test]
    fn metadata_is_fixed_for_builtins() {
        for p in builtin_presets() {
            assert!(p.builtin, "{}", p.id);
            assert_eq!(p.created_at, 0, "{}", p.id);
            assert!(!p.favorite, "{}", p.id);
            assert!(!p.name.trim().is_empty(), "{}", p.id);
            assert!(p.description.as_deref().is_some_and(|d| !d.is_empty()), "{}", p.id);
        }
    }
}
