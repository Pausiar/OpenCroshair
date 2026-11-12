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
