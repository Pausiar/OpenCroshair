# OpenCrosshair architecture (contract for all modules)

Rust 2024, egui/eframe UI, raw Win32 overlay. Everything Windows-specific is `#[cfg(windows)]`
with a no-op/stub fallback so logic + tests build on Linux (`cargo test`), and
`cargo check --target x86_64-pc-windows-gnu` validates the Windows code.

Data flow: `app::App` owns `AppState` (settings, presets, profiles, active crosshair).
UI mutates state. `overlay::OverlayHandle` receives `OverlayCommand`s over an mpsc channel
and runs on its own thread with its own Win32 message loop (decoupled from egui).
Renderer (`crosshair::renderer::render`) is a pure CPU rasterizer -> premultiplied RGBA
`Bitmap`; the SAME function feeds the egui preview texture and the overlay's
UpdateLayeredWindow (per-pixel alpha), guaranteeing fidelity.

Shared types live in `src/crosshair/model.rs` (already written; do not change signatures
without need) and `src/storage/config.rs` (Settings), `src/profiles/mod.rs` (GameProfile).
No DLL injection / memory access / hooking. Detection only uses GetForegroundWindow,
GetWindowThreadProcessId, OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION), QueryFullProcessImageNameW.
