# Building

## Requirements

- **Windows 10 or 11, 64-bit**, for running and packaging the application.
- **Rust stable**, edition 2024 (Rust 1.85 or newer). Install with [rustup](https://rustup.rs/).
- **MSVC build tools**: the "Desktop development with C++" workload from Visual Studio Build Tools, which
  provides the linker used by the `x86_64-pc-windows-msvc` target.
- **Inno Setup 6**, only if you build the installer. Install from <https://jrsoftware.org/isinfo.php> or
  `choco install innosetup`.
- **PowerShell 5.1 or later** (preinstalled on Windows) for the packaging scripts.

Make sure the default target is MSVC:

```powershell
rustup default stable
rustup target add x86_64-pc-windows-msvc
rustup show
```

## Build

```powershell
cargo build                # debug build, target\debug\opencrosshair.exe
cargo build --release      # release build, target\release\opencrosshair.exe
cargo test
```

The release profile uses thin LTO, a single codegen unit, `panic = "abort"` and stripped symbols
(see `Cargo.toml`).

Linting and formatting, as checked by CI:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

`rustfmt.toml` sets the edition and line width, so `cargo fmt` picks them up automatically.

## Packaging

Packaging uses the release binary, so run `cargo build --release` first. Each script takes a version in
`X.Y.Z` form and writes to the `dist\` folder, which is ignored by git.

### Portable ZIP

```powershell
powershell -ExecutionPolicy Bypass -File installer\build-portable.ps1 -Version 0.1.0
```

Produces `dist\OpenCrosshair-0.1.0-portable-win64.zip` containing `opencrosshair.exe`, `LICENSE` and
`README.md` at the top level. The ZIP needs no installation.

### Installer

```powershell
powershell -ExecutionPolicy Bypass -File installer\build-installer.ps1 -Version 0.1.0
```

The script finds `ISCC.exe` on `PATH` or in the default Inno Setup 6 install folder, then runs it on
`installer\OpenCrosshair.iss` with `/DMyAppVersion=<version>`. It produces
`dist\OpenCrosshair-0.1.0-setup.exe`.

Before compiling, the script checks that these inputs exist:

- `target\release\opencrosshair.exe`
- `assets\icons\icon.ico` (used as the installer and uninstaller icon)
- `LICENSE` (shown on the installer's license page)

The installer script itself (`installer\OpenCrosshair.iss`) sets:

- x64 only (`x64compatible`), Windows 10 or newer.
- Per-user install by default (`PrivilegesRequired=lowest`). A dialog allows an all-users install with
  administrator rights.
- Start Menu shortcut always; desktop shortcut optional.
- "Launch at startup" optional and **unticked by default**. When ticked it writes
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\OpenCrosshair` with the value
  `"<install folder>\opencrosshair.exe" --minimized`.
- Uninstall removes program files, shortcuts and the Run value, then asks (default "No") whether to
  delete `%APPDATA%\OpenCrosshair` and `%LOCALAPPDATA%\OpenCrosshair`.

The app version is passed in by the build script and CI; the value in the `.iss` file is only a fallback.

## Continuous integration

Defined in `.github/workflows/`:

- **`ci.yml`** runs on every push to `main` and on pull requests, on `windows-latest`: formatting check,
  clippy with warnings as errors, tests, and a release build.
- **`release.yml`** runs when a tag matching `v*` is pushed (for example `v0.1.0`). It checks that the tag
  version matches the `version` in `Cargo.toml`, builds the release binary, creates the portable ZIP and
  installer, uploads them as workflow artifacts, and attaches them to a GitHub release.

To publish a release:

1. Set `version` in `Cargo.toml` and merge it.
2. Create and push a tag, for example `git tag v0.1.0 && git push origin v0.1.0`.
3. Check the **Release** workflow run and the attached files on the release page.

The release workflow has not yet been run on GitHub. Its first run may need small fixes.
