# Shared brief for every contributor agent
Repo: /home/user/OpenCroshair (Rust 2024 crate `opencrosshair`). Read docs/ARCHITECTURE.md, src/state.rs,
src/crosshair/model.rs, src/storage/config.rs, src/profiles/mod.rs, src/overlay/mod.rs FIRST.
UI visual spec: concept/README.md and concept/*/code.html + screen.png (dark, Linear-like, blue accent).
Palette: bg #101318, surface #181D25, surface2 #202735, blue #3B82F6, blue-light #60A5FA, text #F1F5F9,
text2 #94A3B8, border #2A3340. No emojis in UI. English UI strings.

Rules:
- ONLY edit the files you own (listed in your task). Other agents edit other files concurrently in the same
  tree; if `cargo check` shows errors in files you don't own, ignore them (they are mid-edit) and retry later.
- Keep public signatures from the stubs/contracts; you may ADD public items. If you must change a contract, add a note
  at the end of your final report.
- Real implementations only: no todo!(), no fake data, no dead buttons. Windows-only code under #[cfg(windows)] with
  compiling non-Windows fallbacks so `cargo test` works on Linux. Verify Windows code with
  `cargo check --target x86_64-pc-windows-gnu` (target is installed; linking is not available, so check only).
- Write unit tests for pure logic (#[cfg(test)] in your files). Run `cargo test` / `cargo clippy` for your modules when feasible.
- Do not run `cargo fmt` on the whole project; format only your files with `rustfmt --edition 2024 <file>`.
- Do NOT git commit. Final report: <=15 lines: what you built, public API, caveats.

## Redesign (v0.1.2) additions
Plan: light (cream/light-grey surfaces, light-blue accent, dark-blue primary) + dark (deep navy) themes, serif headings (Source Serif 4) + Inter body,
custom title bar, onboarding, more presets. Palette API contract (src/ui/theme.rs):
`pub struct Palette { bg, surface, surface_alt, border, text, text_muted, text_faint, accent, accent_soft, primary, on_primary, primary_hover, success, warning, danger: Color32 }`,
`pub const LIGHT: Palette`, `pub const DARK: Palette`, `pub fn palette(ctx:&egui::Context)->&'static Palette` (by ctx.style().visuals.dark_mode),
`pub fn apply(ctx:&egui::Context, mode: crate::storage::config::ThemeMode)`. Settings gets `theme: ThemeMode` (default Light) and `onboarding_done: bool`.
Fonts: `ui::fonts::install(ctx)`; families: Proportional=Inter, `FontFamily::Name("semibold".into())`, `FontFamily::Name("serif".into())`.
Pages must never change layout on hover: draw hover actions into fixed rects (painter / ui.put), never by allocating extra widgets inside wrapped rows.
Button labels must be centered both ways.
