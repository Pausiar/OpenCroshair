# OpenCrosshair v0.2.0

A full visual redesign of the app.

## What's new
- New look: cream / light-grey surfaces, light-blue accents and dark-blue primary buttons, with a matching dark theme (toggle in the title bar or Settings).
- Serif headings and Inter body text, bundled with the app (SIL OFL 1.1; licenses are installed under `licenses`).
- Custom title bar with resizable edges; Settings can switch back to the Windows title bar.
- Rebuilt Crosshair Editor with sections, aligned sliders, a fixed preview and an always-visible action bar.
- Preset library with stable tiles (no more jumping on hover) and 22 new original presets.
- First-run welcome flow. Existing settings are migrated automatically.

## Downloads
- `OpenCrosshair-0.2.0-setup.exe` - installer (per-user, no admin needed)
- `OpenCrosshair-0.2.0-portable-win64.zip` - portable build
- `SHA256SUMS` - checksums

## Known limitations / not yet verified
This redesign was built without being able to run the interface on screen during development, so layout details (title bar behaviour, Aero Snap, DPI, fonts) still need checking on a real Windows 10 machine. Overlay click-through, focus, multi-monitor, tray and autostart remain unverified by the author. Exclusive-fullscreen games cannot show the overlay and some anti-cheat systems may block it. The executable is unsigned, so SmartScreen may warn on first run.
