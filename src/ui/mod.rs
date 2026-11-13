pub mod dashboard;
pub mod editor;
pub mod fonts;
pub mod navigation;
pub mod onboarding;
pub mod presets;
pub mod profiles;
pub mod settings;
pub mod theme;
pub mod titlebar;
pub mod widgets;

#[cfg(test)]
mod smoke_tests {
    //! Headless render of every page in both themes: must not panic and must emit geometry.
    use crate::crosshair::presets::builtin_presets;
    use crate::overlay::OverlayHandle;
    use crate::profiles::GameProfile;
    use crate::state::AppState;
    use crate::storage::config::{Settings, ThemeMode};
    use eframe::egui;

    fn state(mode: ThemeMode) -> AppState {
        let settings = Settings {
            theme: mode,
            ..Settings::default()
        };
        AppState {
            editing: settings.active_crosshair.clone(),
            settings,
            presets: builtin_presets(),
            profiles: vec![GameProfile::new("Test game", "game.exe", None)],
            page: Default::default(),
            active_profile: None,
            status: None,
            dirty: false,
            overlay: OverlayHandle::spawn(),
            hotkey_errors: Vec::new(),
            hotkeys_changed: false,
            startup_changed: false,
        }
    }

    type Page = fn(&mut egui::Ui, &mut AppState);

    #[test]
    fn every_page_renders_in_both_themes() {
        let pages: [(&str, Page); 6] = [
            ("overview", super::dashboard::show),
            ("editor", super::editor::show),
            ("presets", super::presets::show),
            ("profiles", super::profiles::show),
            ("settings", super::settings::show),
            ("onboarding", super::onboarding::show),
        ];
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            for (name, page) in pages {
                let ctx = egui::Context::default();
                super::fonts::install(&ctx);
                super::theme::apply(&ctx, mode);
                let mut st = state(mode);
                let mut shapes = 0usize;
                for _ in 0..3 {
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 720.0))),
                        ..Default::default()
                    };
                    let mut out = ctx.run_ui(input, |ui| page(ui, &mut st));
                    shapes += out.shapes.len();
                    out.textures_delta.clear();
                }
                assert!(shapes > 0, "{name} drew nothing in {mode:?}");
            }
        }
    }
}
