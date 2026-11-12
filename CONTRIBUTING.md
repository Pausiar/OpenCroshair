# Contributing to OpenCrosshair

Thanks for considering a contribution. OpenCrosshair is small and aims to stay that way: free, local,
without telemetry, and without touching other programs' memory or input.

## Ground rules

- **Scope.** Changes must keep the project's principles (see the [README](README.md#principles)): no
  telemetry, no network access, no injection into other processes, no reading of game memory, no hooking of
  other programs' input.
- **No bypasses.** Pull requests that try to get around anti-cheat systems, fullscreen restrictions or other
  security mechanisms will not be accepted.
- **No emojis** in the user interface. UI strings are in English.
- **Privacy changes** that alter what is queried or stored must update [docs/PRIVACY.md](docs/PRIVACY.md).
- **File format changes** must update [docs/FILE_FORMAT.md](docs/FILE_FORMAT.md) and follow its versioning
  policy.

## Getting started

1. Install the requirements listed in [docs/BUILDING.md](docs/BUILDING.md).
2. Fork the repository and create a branch from `main`.
3. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing code. It describes how the UI, the
   overlay thread, the renderer and the storage layer fit together.

## Before opening a pull request

Run the same checks CI runs:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

- Add unit tests for new pure logic next to the code (`#[cfg(test)]`).
- Windows-specific code goes under `#[cfg(windows)]`, with a fallback that compiles on other platforms.
  `cargo check --target x86_64-pc-windows-gnu` is a quick way to check the Windows code from another OS.
- Changes that touch the overlay, hotkeys, tray, autostart, detection or the installer must say in the
  pull request that they were **not** tested on Windows unless you actually ran the relevant items in
  [docs/TESTING.md](docs/TESTING.md). Attach the results you observed.

## Pull requests

- Keep each pull request focused on one change.
- Describe what changed and why, and how you tested it.
- Update the README or docs when user-visible behaviour, hotkeys, file formats or build steps change.
- CI must pass before review.

## Commit messages

Short imperative subject line, for example `Fix hotkey conflict message`. Add a body when the reason is not
obvious from the diff.

## Reporting bugs

Open an issue with:

- the OpenCrosshair version (or commit);
- Windows version and build, number of monitors and display scaling;
- the game or program involved, if any, and whether it runs in exclusive fullscreen;
- steps to reproduce and what you expected to happen.

Do not attach personal files, screenshots containing personal information, or your settings folder unless you
have checked them first.

## Licence

By contributing you agree that your contribution is licensed under the MIT License in [LICENSE](LICENSE).
