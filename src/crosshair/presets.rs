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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::validation::validate;
    use std::collections::HashSet;

    #[test]
    fn has_enough_presets_across_all_categories() {
        let presets = builtin_presets();
        assert!(presets.len() >= 12, "only {} presets", presets.len());
        let categories: HashSet<&str> = presets.iter().map(|p| p.category.as_str()).collect();
        for expected in ["Minimal", "Classic", "Dot", "Circle", "Precision", "High visibility"] {
            assert!(categories.contains(expected), "missing category {expected}");
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
