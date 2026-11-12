//! Versioned JSON import/export of crosshairs and preset collections.
//!
//! Format (version 1):
//! - single: `{"format":"opencrosshair","kind":"crosshair","version":1,"preset":{...}}`
//! - collection: `{"format":"opencrosshair","kind":"collection","version":1,"presets":[...]}`
//!
//! Bare `Crosshair` JSON (no envelope, as written by older builds) is also accepted on import.
use crate::crosshair::model::{Crosshair, Preset};
use crate::crosshair::validation;
use crate::storage::config::write_atomic;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

pub const FORMAT_ID: &str = "opencrosshair";
pub const FORMAT_VERSION: u32 = 1;
/// Largest file or text accepted for import.
pub const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
const COLLISION_SUFFIX: &str = " (imported)";
const DEFAULT_IMPORT_NAME: &str = "Imported preset";
const LEGACY_IMPORT_NAME: &str = "Imported crosshair";

/// Field names of `Crosshair`, used to recognise bare legacy crosshair JSON.
const CROSSHAIR_KEYS: &[&str] = &[
    "shape",
    "length_h",
    "length_v",
    "thickness",
    "gap",
    "circle_radius",
    "dot_size",
    "rotation",
    "offset_x",
    "offset_y",
    "show_lines",
    "show_dot",
    "show_circle",
    "color",
    "opacity",
    "outline",
    "outline_thickness",
    "outline_color",
    "shadow",
    "shadow_offset",
    "shadow_opacity",
];

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImportError {
    #[error("the file is not valid JSON: {0}")]
    NotJson(String),
    #[error("this is not an OpenCrosshair file: {0}")]
    WrongFormat(String),
    #[error("file version {found} is not supported (this build reads up to version {max})")]
    UnsupportedVersion { found: u32, max: u32 },
    #[error("the file contains invalid crosshair data: {}", .0.join("; "))]
    Invalid(Vec<String>),
    #[error("the file contains no crosshairs")]
    Empty,
    #[error("the file is larger than the {limit}-byte limit")]
    TooLarge { limit: u64 },
    #[error("could not read the file: {0}")]
    #[allow(dead_code)]
    Io(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportOptions {
    /// Clamp out-of-range values into range instead of rejecting the file.
    pub clamp_out_of_range: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Crosshair,
    Collection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedData {
    pub kind: ImportKind,
    /// Parsed presets with `builtin = false` and fresh ids where the file had none.
    pub presets: Vec<Preset>,
    /// Non-fatal notes, e.g. values that were clamped.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergeReport {
    pub added: usize,
    /// Presets whose id collided and got a new one.
    pub reassigned_ids: usize,
    /// `(original name, new name)` for every renamed preset.
    pub renamed: Vec<(String, String)>,
}

/// Preset as stored in an export file.
#[derive(Serialize, Deserialize)]
struct PresetEntry {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: String,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    created_at: u64,
    crosshair: Crosshair,
}

impl PresetEntry {
    fn from_preset(p: &Preset) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            category: p.category.clone(),
            favorite: p.favorite,
            created_at: p.created_at,
            crosshair: p.crosshair.clone(),
        }
    }

    fn into_preset(self) -> Preset {
        let name = self.name.trim();
        Preset {
            id: if self.id.trim().is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                self.id
            },
            name: if name.is_empty() {
                DEFAULT_IMPORT_NAME.to_string()
            } else {
                name.to_string()
            },
            description: self.description,
            category: if self.category.trim().is_empty() {
                "Custom".into()
            } else {
                self.category
            },
            created_at: self.created_at,
            favorite: self.favorite,
            builtin: false,
            crosshair: self.crosshair,
        }
    }
}

fn envelope(kind: &str, key: &str, body: Value) -> String {
    let mut root = Map::new();
    root.insert("format".into(), FORMAT_ID.into());
    root.insert("kind".into(), kind.into());
    root.insert("version".into(), Value::from(FORMAT_VERSION));
    root.insert(key.into(), body);
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".into())
}

/// Serializes one preset as a single-crosshair export document.
pub fn export_crosshair(preset: &Preset) -> String {
    let entry = serde_json::to_value(PresetEntry::from_preset(preset)).unwrap_or(Value::Null);
    envelope("crosshair", "preset", entry)
}

/// Serializes a list of presets as a collection export document.
pub fn export_collection(presets: &[Preset]) -> String {
    let entries: Vec<Value> = presets
        .iter()
        .map(|p| serde_json::to_value(PresetEntry::from_preset(p)).unwrap_or(Value::Null))
        .collect();
    envelope("collection", "presets", Value::Array(entries))
}

/// Parses and validates an export document, rejecting out-of-range values.
pub fn parse_import(input: &str) -> Result<ImportedData, ImportError> {
    parse_import_with(input, ImportOptions::default())
}

/// Parses and validates an export document with the given options.
pub fn parse_import_with(input: &str, options: ImportOptions) -> Result<ImportedData, ImportError> {
    if input.len() as u64 > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge {
            limit: MAX_IMPORT_BYTES,
        });
    }
    let root: Value = serde_json::from_str(input).map_err(|e| ImportError::NotJson(e.to_string()))?;
    let Value::Object(obj) = &root else {
        return Err(ImportError::WrongFormat("the top level must be a JSON object".into()));
    };

    match obj.get("format") {
        None => {
            if CROSSHAIR_KEYS.iter().any(|k| obj.contains_key(*k)) {
                return parse_legacy_crosshair(root, options);
            }
            return Err(ImportError::WrongFormat("the \"format\" field is missing".into()));
        }
        Some(Value::String(f)) if f == FORMAT_ID => {}
        Some(other) => return Err(ImportError::WrongFormat(format!("unknown format {other}"))),
    }

    let version = obj
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| ImportError::WrongFormat("the \"version\" field is missing".into()))?;
    if version != u64::from(FORMAT_VERSION) {
        return Err(ImportError::UnsupportedVersion {
            found: u32::try_from(version).unwrap_or(u32::MAX),
            max: FORMAT_VERSION,
        });
    }

    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut presets = Vec::new();
    let kind = match obj.get("kind").and_then(Value::as_str) {
        Some("crosshair") => {
            let body = obj.get("preset").cloned().unwrap_or(Value::Null);
            if let Some(p) = decode_entry(body, None, &mut errors) {
                presets.push(p);
            }
            ImportKind::Crosshair
        }
        Some("collection") => {
            match obj.get("presets") {
                Some(Value::Array(items)) => {
                    for (i, item) in items.iter().enumerate() {
                        if let Some(p) = decode_entry(item.clone(), Some(i + 1), &mut errors) {
                            presets.push(p);
                        }
                    }
                }
                _ => return Err(ImportError::WrongFormat("the \"presets\" list is missing".into())),
            }
            ImportKind::Collection
        }
        other => return Err(ImportError::WrongFormat(format!("unknown kind {other:?}"))),
    };

    finish(kind, presets, errors, &mut warnings, options)
}

/// Bare `Crosshair` JSON, as written by earlier builds.
fn parse_legacy_crosshair(root: Value, options: ImportOptions) -> Result<ImportedData, ImportError> {
    let mut errors = Vec::new();
    let mut presets = Vec::new();
    let entry = PresetEntry {
        id: String::new(),
        name: LEGACY_IMPORT_NAME.into(),
        description: None,
        category: String::new(),
        favorite: false,
        created_at: 0,
        crosshair: Crosshair::default(),
    };
    match serde_json::from_value::<Crosshair>(root) {
        Ok(crosshair) => presets.push(PresetEntry { crosshair, ..entry }.into_preset()),
        Err(e) => errors.push(format!("crosshair: {e}")),
    }
    let mut warnings = Vec::new();
    finish(ImportKind::Crosshair, presets, errors, &mut warnings, options)
}

fn decode_entry(value: Value, index: Option<usize>, errors: &mut Vec<String>) -> Option<Preset> {
    match serde_json::from_value::<PresetEntry>(value) {
        Ok(entry) => Some(entry.into_preset()),
        Err(e) => {
            let label = index.map(|i| format!("preset #{i}")).unwrap_or_else(|| "preset".into());
            errors.push(format!("{label}: {e}"));
            None
        }
    }
}

/// Validates every crosshair (rejecting or clamping), then checks the result is not empty.
fn finish(
    kind: ImportKind,
    mut presets: Vec<Preset>,
    mut errors: Vec<String>,
    warnings: &mut Vec<String>,
    options: ImportOptions,
) -> Result<ImportedData, ImportError> {
    for preset in &mut presets {
        match validation::validate(&preset.crosshair) {
            Ok(()) => {}
            Err(problems) if options.clamp_out_of_range => {
                let fields: Vec<&str> = problems.iter().map(|p| p.field).collect();
                validation::clamp(&mut preset.crosshair);
                warnings.push(format!("'{}': clamped {}", preset.name, fields.join(", ")));
            }
            Err(problems) => {
                errors.extend(problems.iter().map(|p| format!("'{}': {p}", preset.name)));
            }
        }
    }
    if !errors.is_empty() {
        return Err(ImportError::Invalid(errors));
    }
    if presets.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(ImportedData {
        kind,
        presets,
        warnings: std::mem::take(warnings),
    })
}

/// Adds `imported` presets to `existing` without overwriting anything. A preset whose id or name
/// (case-insensitive) collides with an existing one gets a new id and a " (imported)" suffix on
/// its name. Existing presets, including built-ins, are never modified.
pub fn merge_imports(existing: &mut Vec<Preset>, imported: Vec<Preset>) -> MergeReport {
    let mut report = MergeReport::default();
    let mut ids: HashSet<String> = existing.iter().map(|p| p.id.clone()).collect();
    let mut names: HashSet<String> = existing.iter().map(|p| p.name.to_lowercase()).collect();

    for mut preset in imported {
        preset.builtin = false;
        let id_clash = ids.contains(&preset.id);
        let name_clash = names.contains(&preset.name.to_lowercase());
        if id_clash || name_clash {
            if id_clash {
                report.reassigned_ids += 1;
            }
            preset.id = uuid::Uuid::new_v4().to_string();
            let original = preset.name.clone();
            preset.name = unique_name(&original, &names);
            report.renamed.push((original, preset.name.clone()));
        }
        ids.insert(preset.id.clone());
        names.insert(preset.name.to_lowercase());
        existing.push(preset);
        report.added += 1;
    }
    report
}

/// `base (imported)`, then `base (imported 2)`, ... whichever is free (compared case-insensitively).
fn unique_name(base: &str, taken: &HashSet<String>) -> String {
    let first = format!("{base}{COLLISION_SUFFIX}");
    if !taken.contains(&first.to_lowercase()) {
        return first;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base} (imported {n})");
        if !taken.contains(&candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

/// Writes an export document to `path` atomically.
#[allow(dead_code)]
pub fn export_to_file(path: &Path, content: &str) -> io::Result<()> {
    write_atomic(path, content.as_bytes())
}

/// Reads and parses an export file (at most `MAX_IMPORT_BYTES`).
#[allow(dead_code)]
pub fn import_from_file(path: &Path) -> Result<ImportedData, ImportError> {
    import_from_file_with(path, ImportOptions::default())
}

#[allow(dead_code)]
pub fn import_from_file_with(path: &Path, options: ImportOptions) -> Result<ImportedData, ImportError> {
    let meta = fs::metadata(path).map_err(|e| ImportError::Io(e.to_string()))?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge {
            limit: MAX_IMPORT_BYTES,
        });
    }
    let bytes = fs::read(path).map_err(|e| ImportError::Io(e.to_string()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| ImportError::NotJson(e.to_string()))?;
    parse_import_with(text, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::{Rgba, limits};
    use tempfile::TempDir;

    fn preset(name: &str) -> Preset {
        let mut p = Preset {
            name: name.into(),
            ..Preset::default()
        };
        p.crosshair.color = Rgba::rgb(10, 20, 30);
        p.crosshair.gap = 7.0;
        p
    }

    fn out_of_range_crosshair() -> Crosshair {
        Crosshair {
            length_h: limits::MAX_LENGTH * 50.0,
            ..Crosshair::default()
        }
    }

    fn invalid_messages(err: ImportError) -> Vec<String> {
        match err {
            ImportError::Invalid(messages) => messages,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn single_crosshair_round_trip() {
        let original = preset("Alpha");
        let text = export_crosshair(&original);
        let imported = parse_import(&text).expect("parse");
        assert_eq!(imported.kind, ImportKind::Crosshair);
        assert_eq!(imported.presets.len(), 1);
        let got = &imported.presets[0];
        assert_eq!(got.id, original.id);
        assert_eq!(got.name, original.name);
        assert_eq!(got.category, original.category);
        assert_eq!(got.crosshair, original.crosshair);
        assert!(!got.builtin);
        assert!(imported.warnings.is_empty());
    }

    #[test]
    fn collection_round_trip() {
        let originals = vec![preset("One"), preset("Two"), preset("Three")];
        let text = export_collection(&originals);
        let imported = parse_import(&text).expect("parse");
        assert_eq!(imported.kind, ImportKind::Collection);
        assert_eq!(imported.presets.len(), 3);
        for (got, want) in imported.presets.iter().zip(&originals) {
            assert_eq!(got.id, want.id);
            assert_eq!(got.name, want.name);
            assert_eq!(got.crosshair, want.crosshair);
        }
    }

    #[test]
    fn export_has_versioned_envelope() {
        let text = export_collection(&[preset("A")]);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["format"], "opencrosshair");
        assert_eq!(value["kind"], "collection");
        assert_eq!(value["version"], 1);
        assert!(value["presets"].is_array());

        let single: Value = serde_json::from_str(&export_crosshair(&preset("B"))).unwrap();
        assert_eq!(single["kind"], "crosshair");
        assert_eq!(single["preset"]["name"], "B");
    }

    #[test]
    fn builtin_flag_is_never_imported() {
        let mut p = preset("Built");
        p.builtin = true;
        let imported = parse_import(&export_crosshair(&p)).unwrap();
        assert!(!imported.presets[0].builtin);
    }

    #[test]
    fn out_of_range_values_are_rejected_with_field_names() {
        let p = Preset {
            crosshair: out_of_range_crosshair(),
            ..preset("Wide")
        };
        let messages = invalid_messages(parse_import(&export_crosshair(&p)).unwrap_err());
        assert!(!messages.is_empty());
        assert!(
            messages.iter().any(|m| m.contains("length_h")),
            "messages: {messages:?}"
        );
        assert!(messages.iter().all(|m| m.contains("Wide")));
    }

    #[test]
    fn out_of_range_values_are_clamped_when_requested() {
        let p = Preset {
            crosshair: out_of_range_crosshair(),
            ..preset("Wide")
        };
        let text = export_crosshair(&p);
        let imported = parse_import_with(
            &text,
            ImportOptions {
                clamp_out_of_range: true,
            },
        )
        .expect("clamped import");
        assert!(imported.presets[0].crosshair.length_h <= limits::MAX_LENGTH);
        assert_eq!(imported.warnings.len(), 1);
        assert!(imported.warnings[0].contains("length_h"));
    }

    #[test]
    fn wrong_format_name_is_rejected() {
        let text = r#"{"format":"someone-else","kind":"crosshair","version":1,"preset":{}}"#;
        assert!(matches!(parse_import(text), Err(ImportError::WrongFormat(_))));
    }

    #[test]
    fn missing_format_without_crosshair_fields_is_rejected() {
        assert!(matches!(
            parse_import(r#"{"hello":1}"#),
            Err(ImportError::WrongFormat(_))
        ));
        assert!(matches!(parse_import("[1,2,3]"), Err(ImportError::WrongFormat(_))));
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let text = r#"{"format":"opencrosshair","kind":"banana","version":1}"#;
        assert!(matches!(parse_import(text), Err(ImportError::WrongFormat(_))));
    }

    #[test]
    fn wrong_version_is_rejected() {
        let text = r#"{"format":"opencrosshair","kind":"collection","version":2,"presets":[]}"#;
        assert_eq!(
            parse_import(text).unwrap_err(),
            ImportError::UnsupportedVersion {
                found: 2,
                max: FORMAT_VERSION
            }
        );
        let missing = r#"{"format":"opencrosshair","kind":"collection","presets":[]}"#;
        assert!(matches!(parse_import(missing), Err(ImportError::WrongFormat(_))));
    }

    #[test]
    fn not_json_is_reported() {
        assert!(matches!(parse_import("{ nope"), Err(ImportError::NotJson(_))));
    }

    #[test]
    fn empty_collection_is_reported() {
        let text = r#"{"format":"opencrosshair","kind":"collection","version":1,"presets":[]}"#;
        assert_eq!(parse_import(text).unwrap_err(), ImportError::Empty);
    }

    #[test]
    fn wrong_field_types_are_reported_as_invalid() {
        let text = r#"{"format":"opencrosshair","kind":"crosshair","version":1,"preset":{"name":"X","crosshair":{"shape":5}}}"#;
        let messages = invalid_messages(parse_import(text).unwrap_err());
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("preset"));
    }

    #[test]
    fn missing_crosshair_is_reported_as_invalid() {
        let text = r#"{"format":"opencrosshair","kind":"crosshair","version":1,"preset":{"name":"X"}}"#;
        assert!(matches!(parse_import(text), Err(ImportError::Invalid(_))));
    }

    #[test]
    fn legacy_bare_crosshair_is_accepted() {
        let text = serde_json::to_string(&Crosshair::default()).unwrap();
        let imported = parse_import(&text).expect("legacy import");
        assert_eq!(imported.kind, ImportKind::Crosshair);
        assert_eq!(imported.presets.len(), 1);
        assert_eq!(imported.presets[0].crosshair, Crosshair::default());
        assert_eq!(imported.presets[0].name, LEGACY_IMPORT_NAME);
    }

    #[test]
    fn oversized_text_is_rejected() {
        let text = " ".repeat(MAX_IMPORT_BYTES as usize + 1);
        assert_eq!(
            parse_import(&text).unwrap_err(),
            ImportError::TooLarge {
                limit: MAX_IMPORT_BYTES
            }
        );
    }

    #[test]
    fn merge_never_overwrites_existing_or_builtin_presets() {
        let builtin = Preset {
            id: "builtin-classic".into(),
            name: "Classic".into(),
            builtin: true,
            ..Preset::default()
        };
        let alpha = preset("Alpha");
        let mut existing = vec![builtin.clone(), alpha.clone()];

        let same_id_other_name = Preset {
            id: alpha.id.clone(),
            name: "Different".into(),
            ..preset("x")
        };
        let builtin_clash = Preset {
            id: "builtin-classic".into(),
            name: "Classic".into(),
            crosshair: out_of_range_crosshair(),
            ..Preset::default()
        };
        let fresh = preset("Gamma");
        let imported = vec![same_id_other_name.clone(), builtin_clash.clone(), fresh.clone()];

        let report = merge_imports(&mut existing, imported);
        assert_eq!(report.added, 3);
        assert_eq!(report.reassigned_ids, 2);
        assert_eq!(existing.len(), 5);
        assert_eq!(existing[0], builtin);
        assert_eq!(existing[1], alpha);

        // Same id -> new id, name kept because it does not collide.
        assert_ne!(existing[2].id, alpha.id);
        assert_eq!(existing[2].name, "Different (imported)");
        assert_ne!(existing[3].id, "builtin-classic");
        assert_eq!(existing[3].name, "Classic (imported)");
        assert!(!existing[3].builtin);
        assert_eq!(existing[4], fresh);
        assert_eq!(report.renamed.len(), 2);
    }

    #[test]
    fn merge_handles_duplicates_within_one_batch() {
        let mut existing = vec![preset("X")];
        let batch = vec![preset("x"), preset("X"), preset("Y")];
        let report = merge_imports(&mut existing, batch);
        let names: Vec<_> = existing.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["X", "x (imported)", "X (imported 2)", "Y"]);
        let ids: HashSet<_> = existing.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), existing.len());
        assert_eq!(report.added, 3);
        assert_eq!(report.reassigned_ids, 0);
    }

    #[test]
    fn export_and_import_files() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("share.json");
        let originals = vec![preset("File A"), preset("File B")];
        export_to_file(&path, &export_collection(&originals)).unwrap();
        let imported = import_from_file(&path).expect("import file");
        assert_eq!(imported.presets.len(), 2);
        assert_eq!(imported.presets[1].name, "File B");
    }

    #[test]
    fn import_from_file_rejects_oversized_files() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("big.json");
        fs::write(&path, vec![b' '; MAX_IMPORT_BYTES as usize + 1]).unwrap();
        assert_eq!(
            import_from_file(&path).unwrap_err(),
            ImportError::TooLarge {
                limit: MAX_IMPORT_BYTES
            }
        );
    }

    #[test]
    fn import_from_missing_file_reports_io() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            import_from_file(&dir.path().join("nope.json")),
            Err(ImportError::Io(_))
        ));
    }
}
