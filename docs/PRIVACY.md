# Privacy

OpenCrosshair is designed to keep everything on your computer. This page describes exactly what
the application looks at, what it keeps, and what it sends anywhere.

## Summary

- No telemetry, analytics, crash reporting or usage statistics.
- No network connections are part of the application's design.
- Game detection reads only which program is in the foreground. It does not read other programs'
  memory, windows' contents, keystrokes or the screen.
- Detection results are compared on your computer and are not stored or transmitted.
- No administrator rights are needed for detection.

## What automatic game detection queries

Automatic profile switching (the "auto profiles" setting, toggled with `Ctrl+Alt+F3` by default) checks
which program is in the foreground at a regular interval (`detection_interval_ms`, 250 to 5000 ms,
default 1000 ms). Each check uses these Windows API calls, in this order:

1. **`GetForegroundWindow`** returns the handle of the window that currently has focus.
2. **`GetWindowThreadProcessId`** returns the process ID (PID) that owns that window.
3. **`OpenProcess`** with the `PROCESS_QUERY_LIMITED_INFORMATION` right opens a handle to that process.
   This is the lowest-privilege right that allows reading a process's executable path. It does not allow
   reading memory, injecting code or changing the process.
4. **`QueryFullProcessImageNameW`** returns the full path of that process's executable file, for example
   `C:\Games\SomeGame\game.exe`.

OpenCrosshair then keeps only the file name (for example `game.exe`), converts it to lower case and
compares it with the executable names you entered in your game profiles. The comparison happens entirely
in memory on your computer.

The application does not:

- read or write the memory of any process;
- inject code or DLLs into any process;
- hook keyboard or mouse input for other programs (global hotkeys are registered through the normal
  Windows hotkey mechanism);
- record window titles, document names, browser addresses or the list of running programs;
- capture screenshots or the contents of other windows.

## What is stored, and where

| Data | Where | When |
|---|---|---|
| Settings (hotkeys, overlay position, detection interval, etc.) | `%APPDATA%\OpenCrosshair` (JSON) | When you change a setting |
| Presets you create or import | `%APPDATA%\OpenCrosshair` (JSON) | When you save |
| Game profiles, including the executable file names you add | `%APPDATA%\OpenCrosshair` (JSON) | When you save |
| Name of the currently active profile | Memory only | While running |
| Foreground process file name checked during detection | Memory only, overwritten on each check | Not written to disk |
| Diagnostic log files (application messages such as file warnings) | `%LOCALAPPDATA%\OpenCrosshair` | While running; kept on your computer only |
| Autostart entry (only if you turn it on) | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | When you turn autostart on or off |

The only information about other programs that is ever written to disk is the executable file name that
**you** type into a game profile. Full paths read during detection are not saved.

## Network

OpenCrosshair is designed not to make any network requests. There is no update checker, no
online preset library and no telemetry. Exported and imported crosshair files are ordinary local files
that you move yourself.

## Permissions

OpenCrosshair runs with normal user rights. The installer installs per user and does not need
administrator rights. Detection requests only the limited-information process right described above. Windows
may refuse even that for some processes, for example programs running elevated (as administrator) or
protected system processes. When that happens the process cannot be identified and a game profile for it
does not match.

## Your control

- Turn automatic profile switching off and detection stops.
- Remove a game profile to stop matching that program.
- Uninstalling offers to delete your data (`%APPDATA%\OpenCrosshair` and `%LOCALAPPDATA%\OpenCrosshair`).
  Choosing "No" keeps it.
