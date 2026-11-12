# Testing

OpenCrosshair has two kinds of testing:

1. **Automated tests** that run with `cargo test`, in CI on Windows and locally.
2. **A manual Windows checklist** that must be run on a real Windows machine.

> **Important:** Most behaviour of this application depends on Windows: the overlay window,
> click-through, focus handling, global hotkeys, the tray icon, autostart, foreground process detection,
> monitors and DPI scaling, and the installer. These **require a real Windows 10 (or 11) machine** to test.
> Automated tests on Linux or in a virtual machine cannot confirm them. Until the checklist below has been
> run and the results reported, treat these behaviours as unverified.

## Automated tests

```powershell
cargo test
```

Unit tests live next to the code they test (`#[cfg(test)]`). They target pure logic: the crosshair data
model and limits, validation and clamping, renderer output, preset handling, settings and profile
serialization, the JSON import/export format (including corrupt and newer-version files) and profile
matching by executable name. Windows-only code is compiled out or replaced by no-op fallbacks on other
platforms, so these tests also run on Linux. Coverage is growing with the code; a module without tests
is not yet covered.

To check that the Windows-only code still compiles from a non-Windows machine:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
```

This checks compilation only. It does not run anything.

Formatting and lints are checked by CI:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Manual Windows test checklist

Use Windows 10 (version 1809 or later, 64-bit). Record the Windows build, the display setup and the
application version in the report. Run the checklist on a clean user profile where possible. Tick each box
only when the item has been observed to pass. Note failures in the report with steps to reproduce.

**Test environment**

- [ ] Windows version and build recorded: ______________________
- [ ] Application version / build recorded: ______________________
- [ ] Display setup recorded (number of monitors, resolutions, scaling): ______________________

### Overlay

- [ ] The crosshair appears in the centre of the selected monitor when the overlay is turned on.
- [ ] Mouse clicks pass through the overlay to the window underneath (click-through works).
- [ ] Turning the overlay on does not take keyboard focus from the active application.
- [ ] The overlay stays on top of windowed and borderless fullscreen games.
- [ ] Changing shape, size, colour, opacity, outline and shadow in the editor updates the overlay immediately.
- [ ] Offsets (X/Y) move the crosshair as expected.
- [ ] Turning the overlay off removes the window completely (nothing remains on screen).

### Multi-monitor

- [ ] Monitor selection places the crosshair on the chosen monitor.
- [ ] Centre on the foreground game window follows a game moved to another monitor.
- [ ] Custom position mode places the crosshair at the chosen screen coordinates.
- [ ] Unplugging the selected monitor falls back to a sensible position and does not crash.
- [ ] Monitors with negative coordinates (placed left of or above the primary) are handled correctly.

### DPI scaling

- [ ] Crosshair is drawn at the correct size and is sharp at **100%** scaling.
- [ ] Crosshair is drawn at the correct size and is sharp at **125%** scaling.
- [ ] Crosshair is drawn at the correct size and is sharp at **150%** scaling.
- [ ] Crosshair is drawn at the correct size and is sharp at **200%** scaling.
- [ ] The settings window is readable and not blurry at each of the scalings above.

### Global hotkeys

- [ ] `Ctrl+Alt+F1` toggles the crosshair on and off.
- [ ] `Ctrl+Alt+F2` toggles the overlay on and off.
- [ ] `Ctrl+Alt+Right` switches to the next preset and the overlay updates.
- [ ] `Ctrl+Alt+Left` switches to the previous preset and the overlay updates.
- [ ] `Ctrl+Alt+F3` toggles automatic game profiles on and off.
- [ ] Hotkeys work while another application (such as a game) has focus.
- [ ] Changing a hotkey in Settings takes effect without restarting the application.

### Hotkey conflicts

- [ ] A hotkey already used by another program is reported as a conflict in Settings > Hotkeys.
- [ ] A conflicting hotkey does not crash the application and the other hotkeys keep working.
- [ ] After choosing a free combination, the new hotkey works.

### Tray

- [ ] The tray icon appears when the application starts.
- [ ] The tray menu opens the main window and quits the application.
- [ ] With "minimize to tray" enabled, closing or minimizing the window keeps the application running.
- [ ] Hotkeys keep working while the main window is closed and only the tray icon remains.

### Autostart

- [ ] With "Launch at startup" unticked during install, no `OpenCrosshair` value exists under
      `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- [ ] Turning autostart on in Settings creates the `OpenCrosshair` Run value, with `--minimized`.
- [ ] After signing out and back in with autostart on, the application starts minimized to the tray.
- [ ] Turning autostart off in Settings removes the Run value.
- [ ] Uninstalling removes the Run value.

### Automatic game profiles

- [ ] Adding a profile with the correct executable name (for example `notepad.exe` as a test) and turning
      auto profiles on applies that profile's preset when the program is brought to the foreground.
- [ ] Switching to a non-matching program returns to the previous or default crosshair.
- [ ] Executable names match regardless of letter case.
- [ ] Disabling a profile stops it from matching.
- [ ] Turning auto profiles off stops switching.
- [ ] An elevated (administrator) program is not matched and causes no error.

### Persistence

- [ ] Settings, presets and profiles are still present after closing and reopening the application.
- [ ] The active preset and overlay position are restored after a restart.
- [ ] Files are written under `%APPDATA%\OpenCrosshair`, and log files under `%LOCALAPPDATA%\OpenCrosshair`.

### Corrupt JSON recovery

- [ ] Replacing `presets.json` with invalid JSON and starting the application does not crash.
- [ ] The corrupt file is renamed with a `.corrupt-<timestamp>` suffix.
- [ ] A warning is shown in the application that the file was corrupt, and defaults are used.
- [ ] The same behaviour occurs for a corrupt settings file and a corrupt profiles file.

### Import and export

- [ ] Exporting a single crosshair produces a file with `"kind": "crosshair"`, as in
      [FILE_FORMAT.md](FILE_FORMAT.md).
- [ ] Exporting a collection produces a file with `"kind": "collection"` containing all selected presets.
- [ ] Importing an exported file restores the same settings and presets.
- [ ] Importing a file with out-of-range values clamps them and reports it.
- [ ] Importing a file with `"version"` higher than supported is refused with a message and nothing is imported.
- [ ] Importing a file that is not OpenCrosshair JSON is refused with a clear error.

### Installer and portable build

- [ ] The installer runs without administrator rights by default and installs under the user profile.
- [ ] Choosing the install-for-all-users option asks for administrator rights.
- [ ] Start Menu shortcut is created and launches the application.
- [ ] The desktop shortcut is created only when its option is ticked.
- [ ] The uninstaller removes program files, shortcuts and the autostart value.
- [ ] The uninstaller asks whether to delete `%APPDATA%\OpenCrosshair` and `%LOCALAPPDATA%\OpenCrosshair`, with "No" as the default, and follows the answer.
- [ ] The portable ZIP runs when extracted to a folder with no installation.

## Reporting results

When reporting a test run, include: the checklist items ticked, every item that failed with steps to
reproduce, the Windows build, the display setup and the application version or commit.
