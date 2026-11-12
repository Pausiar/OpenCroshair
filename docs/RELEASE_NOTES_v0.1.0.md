# OpenCrosshair 0.1.0

A free, open-source crosshair overlay for Windows. It draws a customizable crosshair on top of
your screen, keeps a library of crosshair presets, and can switch the crosshair automatically
when a game you have set up comes to the foreground.

Requires **Windows 10 or newer, 64-bit (x64)**.

## Highlights

- **Overlay** that is transparent, click-through and always on top. It does not take keyboard
  focus from the game.
- **Editor** with a live preview: shape, length, thickness, gap, circle, dot, rotation, offsets,
  colour with alpha, opacity, outline and shadow. Shapes: classic cross, center dot, circle,
  cross + dot, dot ring, T shape and chevron.
- **Presets** library with categories, favourites and a recently used list. Export a single
  crosshair or a whole collection to JSON and import it again.
- **Game profiles** that match a game by its executable file name (for example `cs2.exe`) and
  switch the crosshair automatically when that game is in the foreground.
- **Global hotkeys** to toggle the crosshair and overlay, step through presets and toggle
  automatic profiles without leaving the game.
- **System tray** icon and an optional "Launch at startup" setting (off by default).

## Downloads

Download from the assets section of this release:

- **`OpenCrosshair-0.1.0-setup.exe`** (installer). Installs for your user account and does not
  need administrator rights. You can choose to install for all users instead, which asks for
  administrator rights. Optional: a desktop shortcut and "Launch at startup" (both unticked by
  default).
- **`OpenCrosshair-0.1.0-portable-win64.zip`** (portable). Extract it to any folder and run
  `opencrosshair.exe`. Nothing is written to the Registry for autostart unless you turn it on in
  Settings.

Both versions store settings and presets in your user data folder (see Privacy below).

## Default hotkeys

| Action | Hotkey |
|---|---|
| Toggle crosshair (show / hide) | `Ctrl+Alt+F1` |
| Toggle overlay (on / off) | `Ctrl+Alt+F2` |
| Next preset | `Ctrl+Alt+Right` |
| Previous preset | `Ctrl+Alt+Left` |
| Toggle automatic game profiles | `Ctrl+Alt+F3` |

All hotkeys can be changed in **Settings > Hotkeys**. If another program already uses a
combination, OpenCrosshair reports the conflict there and does not register that action until
you choose a different combination.

For best results, set the game to **borderless fullscreen** or **windowed** mode.

## Privacy summary

- **No telemetry.** No usage data, crash reports or analytics are collected.
- **No network requests.** There is no update checker, online preset library or account.
- **Local files only.** Settings, presets and game profiles are plain JSON under
  `%APPDATA%\OpenCrosshair`. Diagnostic logs are under `%LOCALAPPDATA%\OpenCrosshair`.
- **Only the foreground program's file name is checked**, about once per second, and only when
  automatic profiles are on. The check uses the limited-information process right. The program
  does not read other programs' memory, inject code, hook other programs' input, record window
  titles, or take screenshots. Detection results are not saved.
- **Only the executable names you type into game profiles are written to disk.**
- **Autostart** writes one value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, and
  only if you turn it on. The uninstaller removes it.
- Uninstalling asks whether to delete your data folders. The default answer is "No".

Full details are in [docs/PRIVACY.md](PRIVACY.md).

## Known limitations / not yet verified

**Not yet tested on Windows.** This first release has **not** been tested by the author on a
real Windows 10 machine yet. The pure logic (data model, validation, file format, renderer output)
is covered by automated tests that run on any platform. Behaviour that depends on Windows has not
been verified. This includes:

- overlay click-through, and whether the overlay keeps keyboard focus with the game
- DPI scaling and sharpness at 100%, 125%, 150% and 200%
- multi-monitor placement, including negative monitor coordinates and unplugged monitors
- system tray behaviour and minimize to tray
- hotkey registration and conflict reporting
- autostart (Run key) and the installer and uninstaller

Reports from testers are welcome. The manual checklist is in [docs/TESTING.md](TESTING.md).

**Exclusive fullscreen games cannot be overlaid.** Windows gives exclusive fullscreen programs
direct control of the display, so no other window can be drawn above them. Use borderless
fullscreen or windowed mode.

**Anti-cheat may block or flag the overlay.** Some games' anti-cheat software treats any extra
window on top of the game as suspicious, even though OpenCrosshair does not touch game memory or
input. If an anti-cheat blocks or flags the overlay, stop using it in that game and check the
game's rules before use.

**Elevated and protected programs cannot be identified.** Programs running as administrator, or
with protected status, may not be detected, so a game profile for them will not match.

**Hotkeys can conflict** with other programs that register the same combination.

**Windows SmartScreen warning.** The executable is **not code-signed** in this release. Windows
SmartScreen may show a "Windows protected your PC" warning on first run. To continue, choose
**More info**, then **Run anyway**. Only do this if you downloaded the file from this project's
Releases page.

**Windows 10 or newer, x64 only.** There is no macOS, Linux or 32-bit build.
