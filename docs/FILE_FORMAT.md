# JSON file format

OpenCrosshair exports and imports crosshairs as UTF-8 JSON files. Every file has the same envelope,
followed by a payload that depends on its `kind`.

> **Status:** this page is the format contract. The import/export implementation
> (`src/storage/import_export.rs`) must follow it. If the two disagree, the contract is updated
> first and the version policy below applies.

## Envelope

| Field | Type | Required | Description |
|---|---|---|---|
| `format` | string | yes | Must be exactly `"opencrosshair"`. Other values are rejected. |
| `kind` | string | yes | `"crosshair"` (one crosshair) or `"collection"` (several presets). |
| `version` | integer | yes | Format version. Currently `1`. |

### `kind: "crosshair"`

Holds one crosshair.

```json
{
  "format": "opencrosshair",
  "kind": "crosshair",
  "version": 1,
  "name": "Tight cross",
  "crosshair": {
    "shape": "cross",
    "length_h": 8,
    "length_v": 8,
    "thickness": 2,
    "gap": 3,
    "circle_radius": 12,
    "dot_size": 2,
    "rotation": 0,
    "offset_x": 0,
    "offset_y": 0,
    "show_lines": true,
    "show_dot": false,
    "show_circle": false,
    "color": { "r": 0, "g": 209, "b": 255, "a": 255 },
    "opacity": 1.0,
    "outline": true,
    "outline_thickness": 1,
    "outline_color": { "r": 0, "g": 0, "b": 0, "a": 255 },
    "shadow": false,
    "shadow_offset": 1.5,
    "shadow_opacity": 0.5
  }
}
```

`name` is optional. `crosshair` is required.

### `kind: "collection"`

Holds a list of presets in `presets`. `name` is optional.

```json
{
  "format": "opencrosshair",
  "kind": "collection",
  "version": 1,
  "name": "My presets",
  "presets": [
    {
      "name": "Tight cross",
      "description": "Small cross, thin lines",
      "category": "Custom",
      "crosshair": { "shape": "cross", "length_h": 8, "length_v": 8, "thickness": 2, "gap": 3 }
    }
  ]
}
```

Each entry in `presets`:

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Display name. |
| `description` | string or null | no | Free text. |
| `category` | string | no | Defaults to `"Custom"`. |
| `crosshair` | object | yes | A crosshair object, see below. |

Ids, creation times and favourite flags are local to each installation. They are not exported, and
imported presets always receive a new id. Imported presets are never marked as built-in.

## Crosshair object

Every field is optional. A missing field takes the default value shown in the table. Unknown fields are
ignored, so files from newer versions with extra fields can still be read where the envelope version
allows it.

Lengths, gaps and sizes are in pixels at 100% scale. The ranges come from `crosshair::model::limits`
and the struct defaults in `src/crosshair/model.rs`.

| Field | Type | Default | Allowed range | Notes |
|---|---|---|---|---|
| `shape` | string | `"cross"` | see below | Snake-case enum. |
| `length_h` | number | 10 | 0 to 200 | Horizontal arm length. |
| `length_v` | number | 10 | 0 to 200 | Vertical arm length. |
| `thickness` | number | 2 | 0 to 40 | Line thickness. |
| `gap` | number | 4 | 0 to 100 | Distance from centre to the start of each arm. |
| `circle_radius` | number | 12 | 0 to 200 | Radius of the circle element. |
| `dot_size` | number | 2 | 0 to 60 | Centre dot diameter. |
| `rotation` | number | 0 | 0 to 360 | Degrees. |
| `offset_x` | number | 0 | -2000 to 2000 | Offset from the anchor point. |
| `offset_y` | number | 0 | -2000 to 2000 | Offset from the anchor point. |
| `show_lines` | boolean | true | | Draw the arms. |
| `show_dot` | boolean | false | | Draw the centre dot. |
| `show_circle` | boolean | false | | Draw the circle. |
| `color` | Rgba object | `{r:0,g:209,b:255,a:255}` | each channel 0 to 255 | Fill colour. |
| `opacity` | number | 1.0 | 0.0 to 1.0 | Global opacity multiplier. |
| `outline` | boolean | true | | Draw an outline around the shapes. |
| `outline_thickness` | number | 1 | 0 to 10 | |
| `outline_color` | Rgba object | `{r:0,g:0,b:0,a:255}` | each channel 0 to 255 | |
| `shadow` | boolean | false | | Draw a drop shadow. |
| `shadow_offset` | number | 1.5 | 0 or more | Shadow distance in pixels. No upper bound is defined yet. |
| `shadow_opacity` | number | 0.5 | 0.0 to 1.0 | |

Values outside the allowed range are clamped on import. A value that cannot be read at all (for example a
string where a number is expected, or a colour channel above 255) makes the import fail with an error that
names the field.

### `shape` values

`cross`, `dot`, `circle`, `cross_dot`, `cross_no_outer`, `t_shape`, `chevron`

These correspond to the UI labels Classic cross, Center dot, Circle, Cross + dot, Dot ring, T shape
and Chevron.

### Rgba object

```json
{ "r": 0, "g": 209, "b": 255, "a": 255 }
```

All four channels are integers from 0 to 255. `a` is alpha, where 255 is fully opaque.

## Versioning and compatibility

- `version` is a single integer for the whole format. It is independent of the application version and
  of the settings file version.
- **Additive changes stay at the same version.** A new optional field with a default is added without
  bumping `version`. Older versions of the application ignore it.
- **Breaking changes bump `version`.** Renaming or removing a field, changing a field's meaning or
  changing an enum's values is a breaking change.
- **Importers reject newer versions.** A file with `version` greater than the highest version the application
  supports is refused with a message asking the user to update OpenCrosshair. It is never partially imported.
- **Older versions are migrated.** When a new version is introduced, the importer keeps reading the
  older versions it supports.
- Files that do not have `format` set to `"opencrosshair"`, or whose `kind` is unknown, are rejected.
