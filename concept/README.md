# Open Crosshair — UI concepts

Visual direction for the Windows desktop application.

## Direction

- **Theme:** dark
- **Accent:** blue
- **Design language:** Linear-inspired, clean and restrained
- **Density:** balanced
- **Target:** Windows desktop (Windows 10/11)

## Screens

| Preview | Description |
|---|---|
| [01 — Editor](01-editor.svg) | Live crosshair preview and visual controls for size, thickness, gap, opacity, colour, outline and positioning. |
| [02 — Dashboard](02-dashboard.svg) | Overview of the active crosshair, recent presets, game profiles and quick actions. |
| [03 — Preset library](03-preset-library.svg) | Searchable grid of built-in and custom crosshair presets, with favourites and import/export actions. |
| [04 — Game profiles](04-game-profiles.svg) | Profiles assigned per game, with profile-level crosshair settings and position/display controls. |

## Suggested design tokens

| Token | Value |
|---|---|
| App background | `#0B101A` |
| Surface | `#151D2B` |
| Elevated surface | `#1B2638` |
| Primary blue | `#1683F8` |
| Crosshair cyan | `#00D1FF` |
| Main text | `#F4F7FC` |
| Secondary text | `#A7B3C5` |
| Border | `#2A3548` |
| Success | `#27D980` |

## UX notes

- Keep the crosshair preview visible while editing.
- Make size, thickness, gap, opacity, colour and X/Y position easy to tune.
- Support global and per-game profiles.
- Make preset import/export discoverable without crowding the editor.
- Use the same sidebar and navigation model across all workspaces.

> These images are exploratory UI concepts, not production screenshots or a final implementation specification.
