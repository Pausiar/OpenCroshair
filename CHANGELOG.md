# Changelog

All notable changes to OpenCrosshair are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-11-13

### Fixed
- Release builds no longer open a console window behind the app.
- The window and taskbar icon now use the full 256 px application icon.

## [0.1.0] - 2026-11-12

First public release. Windows 10 or newer, 64-bit (x64) only.

Note: the Windows-specific behaviour of this release has not yet been verified on a real
Windows 10 machine. See the known limitations in the release notes.

### Added

- Transparent, click-through, always-on-top overlay window that draws the crosshair without
  taking keyboard focus from the active application.
- Editor with a live preview: shape, horizontal and vertical length, thickness, gap, circle
  radius, dot size, rotation, X/Y offset, element toggles (lines, center dot, circle), colour
  with alpha, opacity, outline and shadow.
- Shapes: classic cross, center dot, circle, cross + dot, dot ring, T shape and chevron.
- Positioning: centre on a chosen monitor, on the foreground game window, or at a custom
  screen position, with X/Y offsets.
- Preset library with names, categories, descriptions, favourites and a recently used list.
  Built-in presets and user presets are shown side by side.
- JSON import and export for a single crosshair or a whole collection of presets, in a
  versioned file format. Importing clamps out-of-range values and refuses files with a higher
  version than supported.
- Game profiles: assign a preset to a game by its executable file name (for example
  `cs2.exe`), with case-insensitive matching.
- Automatic profile switching that checks the foreground program about once per second
  (configurable from 250 to 5000 ms) and applies the matching preset. It uses only the
  limited-information process right; no memory, input or window contents are read.
- Global hotkeys, changeable in Settings > Hotkeys, with conflict reporting when another
  program already uses a combination:
  - Toggle crosshair: `Ctrl+Alt+F1`
  - Toggle overlay: `Ctrl+Alt+F2`
  - Next preset: `Ctrl+Alt+Right`
  - Previous preset: `Ctrl+Alt+Left`
  - Toggle automatic game profiles: `Ctrl+Alt+F3`
- System tray icon with quick access to the main window and quit. Optional minimize to tray.
- Optional "Launch at startup" (off by default), which starts OpenCrosshair minimized to the
  tray through the per-user Run key.
- Start-minimized setting, also applied when launched with `--minimized`.
- Local storage of settings, presets and profiles as JSON under `%APPDATA%\OpenCrosshair`,
  with diagnostic logs under `%LOCALAPPDATA%\OpenCrosshair`.
- Corrupt-file recovery: a damaged settings, preset or profile file is renamed with a
  `.corrupt-<timestamp>` suffix, defaults are used, and the application shows a warning.
- Inno Setup installer (per-user by default, optional all-users install, optional desktop
  shortcut and startup entry) and a portable ZIP build.
- Automated tests (`cargo test`) for the data model, validation, renderer output, presets,
  serialization, import/export and profile matching, run in CI.

### Security and privacy

- No telemetry, analytics, crash reporting or network requests.
- No code injection, no reading or writing of other processes' memory, and no hooking of
  other programs' input.
