//! Bundled UI fonts: Inter (body, UI labels) and Source Serif 4 (headings).
//!
//! Font files live in `assets/fonts/` and are subset to Latin, Latin-1, Latin Extended-A and
//! general punctuation. All are licensed under the SIL Open Font License 1.1 (see the OFL-*.txt
//! files next to them).
//!
//! Families after [`install`]:
//! - `FontFamily::Proportional`: Inter Regular first, then egui's built-in fonts as fallback.
//! - `FontFamily::Name("semibold")`: Inter SemiBold, then Inter Regular.
//! - `FontFamily::Name("serif")`: Source Serif 4 SemiBold, then Inter Regular.
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/inter-regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/inter-semibold.ttf");
const SOURCE_SERIF_4_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/source-serif-4-semibold.ttf");

const INTER: &str = "inter";
const INTER_SEMIBOLD_KEY: &str = "inter-semibold";
const SOURCE_SERIF_KEY: &str = "source-serif-4-semibold";

/// Family name used for SemiBold UI text (`FontFamily::Name("semibold")`).
pub const SEMIBOLD: &str = "semibold";
/// Family name used for serif headings (`FontFamily::Name("serif")`).
pub const SERIF: &str = "serif";

/// Register the bundled fonts on the context. Call once at startup, before the first frame.
#[allow(dead_code)] // Not yet called from app.rs; remove this attribute once it is wired in.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

/// Build the font definitions: bundled fonts layered over egui's defaults.
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();

    defs.font_data
        .insert(INTER.to_owned(), Arc::new(FontData::from_static(INTER_REGULAR)));
    defs.font_data.insert(
        INTER_SEMIBOLD_KEY.to_owned(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    defs.font_data.insert(
        SOURCE_SERIF_KEY.to_owned(),
        Arc::new(FontData::from_static(SOURCE_SERIF_4_SEMIBOLD)),
    );

    // Proportional: Inter first, keep egui's defaults behind it for glyphs Inter lacks.
    let proportional = defs.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, INTER.to_owned());

    defs.families.insert(
        FontFamily::Name(SEMIBOLD.into()),
        vec![INTER_SEMIBOLD_KEY.to_owned(), INTER.to_owned()],
    );
    defs.families.insert(
        FontFamily::Name(SERIF.into()),
        vec![SOURCE_SERIF_KEY.to_owned(), INTER.to_owned()],
    );

    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_starts_with_inter() {
        let defs = definitions();
        let first = defs.families[&FontFamily::Proportional].first().map(String::as_str);
        assert_eq!(first, Some(INTER));
    }

    #[test]
    fn named_families_resolve_to_bundled_fonts() {
        let defs = definitions();
        assert_eq!(
            defs.families[&FontFamily::Name(SEMIBOLD.into())],
            vec![INTER_SEMIBOLD_KEY.to_owned(), INTER.to_owned()]
        );
        assert_eq!(
            defs.families[&FontFamily::Name(SERIF.into())],
            vec![SOURCE_SERIF_KEY.to_owned(), INTER.to_owned()]
        );
        for key in [INTER, INTER_SEMIBOLD_KEY, SOURCE_SERIF_KEY] {
            assert!(defs.font_data.contains_key(key), "missing font data for {key}");
        }
    }
}
