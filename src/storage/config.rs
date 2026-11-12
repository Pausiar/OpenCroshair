//! Settings + on-disk layout: data paths, versioned JSON load/save with corrupt-file recovery,
//! atomic writes and a background save worker.
use crate::crosshair::model::{Crosshair, Preset};
use crate::crosshair::presets::builtin_presets;
use crate::crosshair::validation;
use crate::profiles::GameProfile;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SETTINGS_VERSION: u32 = 1;
pub const PRESETS_VERSION: u32 = 1;
pub const PROFILES_VERSION: u32 = 1;

pub const SETTINGS_FILE_NAME: &str = "settings.json";
pub const PRESETS_FILE_NAME: &str = "presets.json";
pub const PROFILES_FILE_NAME: &str = "profiles.json";

pub const MIN_DETECTION_INTERVAL_MS: u32 = 250;
pub const MAX_DETECTION_INTERVAL_MS: u32 = 5000;
pub const MIN_PREVIEW_ZOOM: f32 = 1.0;
pub const MAX_PREVIEW_ZOOM: f32 = 16.0;
const DEFAULT_PREVIEW_ZOOM: f32 = 4.0;
const MAX_RECENT_PRESETS: usize = 8;
/// Upper bound on errors kept for the UI; older ones are dropped.
const MAX_STORED_ERRORS: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    /// Strings like "Ctrl+Alt+F1" (parsed by hotkeys module).
    pub toggle_crosshair: String,
    pub toggle_overlay: String,
    pub next_preset: String,
    pub prev_preset: String,
    pub toggle_auto_profiles: String,
}
impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            toggle_crosshair: "Ctrl+Alt+F1".into(),
            toggle_overlay: "Ctrl+Alt+F2".into(),
            next_preset: "Ctrl+Alt+Right".into(),
            prev_preset: "Ctrl+Alt+Left".into(),
            toggle_auto_profiles: "Ctrl+Alt+F3".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CenterMode {
    /// Center of the chosen monitor.
    #[default]
    Monitor,
    /// Center of the foreground game window (falls back to monitor).
    GameWindow,
    /// Absolute screen position (custom_x/custom_y).
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PositionSettings {
    pub mode: CenterMode,
    /// Monitor index as enumerated by system::monitors (None = primary).
    pub monitor: Option<usize>,
    pub offset_x: i32,
    pub offset_y: i32,
    pub custom_x: i32,
    pub custom_y: i32,
}
impl Default for PositionSettings {
    fn default() -> Self {
        Self { mode: CenterMode::Monitor, monitor: None, offset_x: 0, offset_y: 0, custom_x: 0, custom_y: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Crosshair currently applied (editor "Apply").
    pub active_crosshair: Crosshair,
    pub active_preset_id: Option<String>,
    pub crosshair_visible: bool,
    pub overlay_enabled: bool,
    pub position: PositionSettings,
    pub hotkeys: HotkeySettings,
    pub auto_profiles: bool,
    /// Poll interval (ms) for foreground detection, 250..=5000.
    pub detection_interval_ms: u32,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub launch_at_startup: bool,
    pub default_profile_preset_id: Option<String>,
    pub preview_dark_background: bool,
    pub preview_zoom: f32,
    pub recent_preset_ids: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            active_crosshair: Crosshair::default(),
            active_preset_id: None,
            crosshair_visible: true,
            overlay_enabled: false,
            position: PositionSettings::default(),
            hotkeys: HotkeySettings::default(),
            auto_profiles: false,
            detection_interval_ms: 1000,
            minimize_to_tray: true,
            start_minimized: false,
            launch_at_startup: false,
            default_profile_preset_id: None,
            preview_dark_background: true,
            preview_zoom: 4.0,
            recent_preset_ids: Vec::new(),
        }
    }
}

/// Resolved data directories (config, presets, profiles, logs).
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

/// Everything loaded from disk. Corrupt files are renamed `*.corrupt-<ts>` and defaults used.
#[derive(Debug, Default)]
pub struct LoadedData {
    pub settings: Settings,
    pub presets: Vec<Preset>,
    pub profiles: Vec<GameProfile>,
    /// Human readable warnings to show in UI (e.g. "presets.json was corrupt").
    pub warnings: Vec<String>,
}

impl Paths {
    /// Platform locations from `directories::ProjectDirs`. `None` when no home directory exists.
    /// Settings live in the config dir, presets and profiles in the data dir, logs in
    /// `<local data dir>/logs`.
    pub fn resolve() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("io", "OpenCrosshair", "OpenCrosshair")?;
        Some(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            log_dir: dirs.data_local_dir().join("logs"),
        })
    }

    /// Self-contained layout below `root` (used by tests and portable setups).
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self { config_dir: root.join("config"), data_dir: root.join("data"), log_dir: root.join("logs") }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE_NAME)
    }

    pub fn presets_file(&self) -> PathBuf {
        self.data_dir.join(PRESETS_FILE_NAME)
    }

    pub fn profiles_file(&self) -> PathBuf {
        self.data_dir.join(PROFILES_FILE_NAME)
    }
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

/// Upgrades a raw settings document to `SETTINGS_VERSION`, one step at a time.
/// A document without a version is treated as version 0. Documents from a newer build are
/// returned as-is (best effort); `load_all` reports those as a warning.
pub fn migrate_settings(value: Value) -> Value {
    let mut warnings = Vec::new();
    migrate_settings_checked(value, &mut warnings)
}

fn migrate_settings_checked(mut value: Value, warnings: &mut Vec<String>) -> Value {
    if let Some(obj) = value.as_object_mut() {
        let found = obj.get("version").and_then(Value::as_u64).unwrap_or(0);
        if found > u64::from(SETTINGS_VERSION) {
            warnings.push(format!(
                "settings.json was written by a newer version (format {found}, supported {SETTINGS_VERSION}); loaded on a best-effort basis"
            ));
            return value;
        }
        let mut current = found as u32;
        while current < SETTINGS_VERSION {
            current = match current {
                0 => migrate_v0_to_v1(obj),
                _ => SETTINGS_VERSION,
            };
        }
    }
    value
}

/// Version 0 had no `version` key; its field layout is identical to version 1.
fn migrate_v0_to_v1(obj: &mut serde_json::Map<String, Value>) -> u32 {
    obj.insert("version".to_string(), Value::from(1u32));
    1
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Reads and parses a JSON file. Missing file -> `None` silently. Unreadable file -> `None` with
/// a warning. Invalid JSON -> the file is quarantined, `None` returned.
fn read_json_file(path: &Path, warnings: &mut Vec<String>) -> Option<Value> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            warnings.push(format!("Could not read {}: {e}. Defaults were loaded.", path.display()));
            return None;
        }
    };
    // Tolerate a UTF-8 byte order mark (some editors add one).
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => Some(value),
        Err(e) => {
            quarantine(path, &format!("invalid JSON: {e}"), warnings);
            None
        }
    }
}

/// Moves a corrupt file aside as `<name>.corrupt-<unix_ts>` (adding a counter if needed).
fn quarantine(path: &Path, reason: &str, warnings: &mut Vec<String>) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ts = unix_now();
    let mut target = path.with_file_name(format!("{name}.corrupt-{ts}"));
    let mut n = 1;
    while target.exists() {
        n += 1;
        target = path.with_file_name(format!("{name}.corrupt-{ts}-{n}"));
    }
    match fs::rename(path, &target) {
        Ok(()) => {
            tracing::warn!("{name} is unreadable ({reason}); moved to {}", target.display());
            warnings.push(format!(
                "{name} was unreadable ({reason}). It was moved to {} and defaults were loaded.",
                target.display()
            ));
        }
        Err(e) => {
            tracing::warn!("{name} is unreadable ({reason}) and could not be moved: {e}");
            warnings.push(format!(
                "{name} was unreadable ({reason}) and could not be moved aside: {e}. Defaults were loaded."
            ));
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Clamps every settings value into its supported range.
fn sanitize_settings(s: &mut Settings) {
    s.version = SETTINGS_VERSION;
    validation::clamp(&mut s.active_crosshair);
    s.detection_interval_ms = s.detection_interval_ms.clamp(MIN_DETECTION_INTERVAL_MS, MAX_DETECTION_INTERVAL_MS);
    s.preview_zoom = if s.preview_zoom.is_finite() {
        s.preview_zoom.clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM)
    } else {
        DEFAULT_PREVIEW_ZOOM
    };
    let mut seen = HashSet::new();
    s.recent_preset_ids.retain(|id| seen.insert(id.clone()));
    s.recent_preset_ids.truncate(MAX_RECENT_PRESETS);
}

fn load_settings(paths: &Paths, warnings: &mut Vec<String>) -> Settings {
    let path = paths.settings_file();
    let Some(value) = read_json_file(&path, warnings) else {
        return Settings::default();
    };
    let value = migrate_settings_checked(value, warnings);
    match serde_json::from_value::<Settings>(value) {
        Ok(mut settings) => {
            sanitize_settings(&mut settings);
            settings
        }
        Err(e) => {
            quarantine(&path, &format!("unexpected structure: {e}"), warnings);
            Settings::default()
        }
    }
}

/// Extracts a list stored either as a bare JSON array or under `key` of an object.
fn items_of(value: Value, key: &str) -> Option<Vec<Value>> {
    match value {
        Value::Array(items) => Some(items),
        Value::Object(mut obj) => match obj.remove(key) {
            Some(Value::Array(items)) => Some(items),
            _ => None,
        },
        _ => None,
    }
}

/// Decodes each item separately so one bad entry does not discard the whole list.
fn decode_items<T: serde::de::DeserializeOwned>(items: Vec<Value>, what: &str, warnings: &mut Vec<String>) -> Vec<T> {
    items
        .into_iter()
        .enumerate()
        .filter_map(|(i, value)| match serde_json::from_value::<T>(value) {
            Ok(item) => Some(item),
            Err(e) => {
                warnings.push(format!("Skipped {what} #{} because it could not be read: {e}", i + 1));
                None
            }
        })
        .collect()
}

/// Loads user presets. Built-in flags are forced off, ids must be non-empty and unique
/// (`reserved_ids` holds ids already taken by built-ins or earlier entries).
fn load_user_presets(paths: &Paths, reserved_ids: &mut HashSet<String>, warnings: &mut Vec<String>) -> Vec<Preset> {
    let path = paths.presets_file();
    let Some(value) = read_json_file(&path, warnings) else {
        return Vec::new();
    };
    let Some(items) = items_of(value, "presets") else {
        quarantine(&path, "expected a list of presets", warnings);
        return Vec::new();
    };
    let decoded: Vec<Preset> = decode_items(items, "preset", warnings);
    let mut out = Vec::with_capacity(decoded.len());
    for mut preset in decoded {
        preset.builtin = false;
        if preset.id.trim().is_empty() {
            preset.id = uuid::Uuid::new_v4().to_string();
        } else if reserved_ids.contains(&preset.id) {
            warnings.push(format!("Preset '{}' reused an existing id and was given a new one.", preset.name));
            preset.id = uuid::Uuid::new_v4().to_string();
        }
        reserved_ids.insert(preset.id.clone());
        validation::clamp(&mut preset.crosshair);
        if preset.name.trim().is_empty() {
            preset.name = "Untitled".into();
        }
        out.push(preset);
    }
    out
}

fn load_profiles(paths: &Paths, warnings: &mut Vec<String>) -> Vec<GameProfile> {
    let path = paths.profiles_file();
    let Some(value) = read_json_file(&path, warnings) else {
        return Vec::new();
    };
    let Some(items) = items_of(value, "profiles") else {
        quarantine(&path, "expected a list of profiles", warnings);
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out: Vec<GameProfile> = decode_items(items, "profile", warnings);
    for profile in &mut out {
        if profile.id.trim().is_empty() || !seen.insert(profile.id.clone()) {
            profile.id = uuid::Uuid::new_v4().to_string();
            seen.insert(profile.id.clone());
        }
    }
    out
}

/// Loads settings, presets and profiles. Missing files give defaults silently; corrupt files are
/// moved aside and reported in `warnings`. The returned presets are the built-in presets first,
/// then the user presets.
pub fn load_all(paths: &Paths) -> LoadedData {
    let mut warnings = Vec::new();
    let settings = load_settings(paths, &mut warnings);

    let mut presets = builtin_presets();
    for preset in &mut presets {
        preset.builtin = true;
    }
    let mut reserved: HashSet<String> = presets.iter().map(|p| p.id.clone()).collect();
    let user = load_user_presets(paths, &mut reserved, &mut warnings);
    presets.extend(user);

    let profiles = load_profiles(paths, &mut warnings);
    LoadedData { settings, presets, profiles, warnings }
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PresetsFile<'a> {
    version: u32,
    presets: Vec<&'a Preset>,
}

#[derive(Serialize)]
struct ProfilesFile<'a> {
    version: u32,
    profiles: &'a [GameProfile],
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` atomically: a temp file in the same directory is written, fsynced and
/// renamed over the target, so readers see either the old or the new complete file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)?;
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".{}.{}.{seq}.tmp", file_name.to_string_lossy(), std::process::id()));

    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    sync_dir(dir);
    Ok(())
}

#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

pub fn save_settings(paths: &Paths, settings: &Settings) -> io::Result<()> {
    let mut to_write = settings.clone();
    to_write.version = SETTINGS_VERSION;
    let bytes = serde_json::to_vec_pretty(&to_write).map_err(io::Error::other)?;
    write_atomic(&paths.settings_file(), &bytes)
}

/// Saves only the non-built-in presets; built-ins are regenerated on every load.
pub fn save_presets(paths: &Paths, presets: &[Preset]) -> io::Result<()> {
    let file = PresetsFile {
        version: PRESETS_VERSION,
        presets: presets.iter().filter(|p| !p.builtin).collect(),
    };
    let bytes = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
    write_atomic(&paths.presets_file(), &bytes)
}

pub fn save_profiles(paths: &Paths, profiles: &[GameProfile]) -> io::Result<()> {
    let file = ProfilesFile { version: PROFILES_VERSION, profiles };
    let bytes = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
    write_atomic(&paths.profiles_file(), &bytes)
}

// ---------------------------------------------------------------------------
// Background save worker
// ---------------------------------------------------------------------------

/// A set of documents to write. `None` fields are left untouched on disk.
#[derive(Debug, Clone, Default)]
pub struct SaveJob {
    pub settings: Option<Settings>,
    pub presets: Option<Vec<Preset>>,
    pub profiles: Option<Vec<GameProfile>>,
}

impl SaveJob {
    /// Folds a newer job into this one; the newer data wins per document.
    fn merge(&mut self, newer: SaveJob) {
        if newer.settings.is_some() {
            self.settings = newer.settings;
        }
        if newer.presets.is_some() {
            self.presets = newer.presets;
        }
        if newer.profiles.is_some() {
            self.profiles = newer.profiles;
        }
    }
}

enum WorkerMsg {
    Job(SaveJob),
    /// Acknowledged once every job queued before it has been written.
    Flush(mpsc::Sender<()>),
}

/// Owns a thread that writes jobs to disk. Queued jobs are coalesced so only the newest state of
/// each document is written. Errors are kept for the UI to poll with `take_errors`. Dropping the
/// worker writes everything still queued before returning.
pub struct SaveWorker {
    tx: Option<mpsc::Sender<WorkerMsg>>,
    handle: Option<JoinHandle<()>>,
    errors: Arc<Mutex<Vec<String>>>,
}

impl SaveWorker {
    pub fn spawn(paths: Paths) -> Self {
        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let errors = Arc::new(Mutex::new(Vec::new()));
        let worker_errors = Arc::clone(&errors);
        let spawned = std::thread::Builder::new()
            .name("opencrosshair-save".into())
            .spawn(move || run_worker(paths, rx, worker_errors));
        let handle = match spawned {
            Ok(handle) => Some(handle),
            Err(e) => {
                push_error(&errors, format!("Could not start the save thread: {e}"));
                None
            }
        };
        Self { tx: Some(tx), handle, errors }
    }

    /// Queues a job. Never blocks on disk I/O.
    pub fn save(&self, job: SaveJob) {
        let sent = match &self.tx {
            Some(tx) => tx.send(WorkerMsg::Job(job)).is_ok(),
            None => false,
        };
        if !sent {
            push_error(&self.errors, "Changes could not be saved because the save thread stopped.".into());
        }
    }

    /// Blocks until every job queued so far has been written.
    pub fn flush(&self) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = mpsc::channel();
            if tx.send(WorkerMsg::Flush(ack_tx)).is_ok() {
                let _ = ack_rx.recv();
            }
        }
    }

    /// Returns and clears save errors reported since the last call.
    pub fn take_errors(&self) -> Vec<String> {
        let mut guard = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *guard)
    }
}

impl Drop for SaveWorker {
    fn drop(&mut self) {
        // Closing the channel lets the worker drain what is queued, then exit.
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_worker(paths: Paths, rx: mpsc::Receiver<WorkerMsg>, errors: Arc<Mutex<Vec<String>>>) {
    while let Ok(msg) = rx.recv() {
        let mut job = match msg {
            WorkerMsg::Job(job) => job,
            WorkerMsg::Flush(ack) => {
                let _ = ack.send(());
                continue;
            }
        };
        let mut acks = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(WorkerMsg::Job(newer)) => job.merge(newer),
                Ok(WorkerMsg::Flush(ack)) => {
                    // Write what we have so far before acknowledging the flush.
                    acks.push(ack);
                    break;
                }
                Err(_) => break,
            }
        }
        write_job(&paths, job, &errors);
        for ack in acks {
            let _ = ack.send(());
        }
    }
}

fn write_job(paths: &Paths, job: SaveJob, errors: &Mutex<Vec<String>>) {
    if let Some(settings) = job.settings
        && let Err(e) = save_settings(paths, &settings)
    {
        push_error(errors, format!("Could not save settings: {e}"));
    }
    if let Some(presets) = job.presets
        && let Err(e) = save_presets(paths, &presets)
    {
        push_error(errors, format!("Could not save presets: {e}"));
    }
    if let Some(profiles) = job.profiles
        && let Err(e) = save_profiles(paths, &profiles)
    {
        push_error(errors, format!("Could not save profiles: {e}"));
    }
}

fn push_error(errors: &Mutex<Vec<String>>, message: String) {
    tracing::error!("{message}");
    let mut guard = errors.lock().unwrap_or_else(|e| e.into_inner());
    guard.push(message);
    let len = guard.len();
    if len > MAX_STORED_ERRORS {
        guard.drain(..len - MAX_STORED_ERRORS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::Rgba;
    use serde_json::json;
    use tempfile::TempDir;

    fn temp_paths() -> (TempDir, Paths) {
        let dir = TempDir::new().expect("tempdir");
        let paths = Paths::from_root(dir.path());
        (dir, paths)
    }

    fn user_preset(id: &str, name: &str) -> Preset {
        Preset { id: id.into(), name: name.into(), ..Preset::default() }
    }

    fn quarantined_files(dir: &Path, base: &str) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{base}.corrupt-")))
            .collect()
    }

    #[test]
    fn resolve_yields_distinct_log_dir() {
        let paths = Paths::resolve().expect("project directories");
        assert!(paths.log_dir.ends_with("logs"));
        assert!(paths.config_dir.ends_with("OpenCrosshair") || paths.config_dir.ends_with("config"));
    }

    #[test]
    fn from_root_builds_layout() {
        let paths = Paths::from_root("/x");
        assert_eq!(paths.settings_file(), Path::new("/x/config/settings.json"));
        assert_eq!(paths.presets_file(), Path::new("/x/data/presets.json"));
        assert_eq!(paths.profiles_file(), Path::new("/x/data/profiles.json"));
        assert_eq!(paths.log_dir, Path::new("/x/logs"));
    }

    #[test]
    fn missing_files_yield_defaults_without_warnings() {
        let (_dir, paths) = temp_paths();
        let loaded = load_all(&paths);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.presets.len(), builtin_presets().len());
        assert!(loaded.presets.iter().all(|p| p.builtin));
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn corrupt_settings_are_quarantined_and_defaults_used() {
        let (dir, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.settings_file(), "{ this is not json").unwrap();

        let loaded = load_all(&paths);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.warnings.len(), 1, "warnings: {:?}", loaded.warnings);
        assert!(loaded.warnings[0].contains("settings.json"));
        assert!(!paths.settings_file().exists());
        let moved = quarantined_files(&paths.config_dir, "settings.json");
        assert_eq!(moved.len(), 1);
        assert_eq!(fs::read_to_string(&moved[0]).unwrap(), "{ this is not json");
        drop(dir);
    }

    #[test]
    fn corrupt_presets_and_profiles_are_quarantined() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::write(paths.presets_file(), "[1, 2").unwrap();
        fs::write(paths.profiles_file(), "").unwrap();

        let loaded = load_all(&paths);
        assert!(loaded.presets.iter().all(|p| p.builtin));
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.warnings.len(), 2, "warnings: {:?}", loaded.warnings);
        assert_eq!(quarantined_files(&paths.data_dir, "presets.json").len(), 1);
        assert_eq!(quarantined_files(&paths.data_dir, "profiles.json").len(), 1);
    }

    #[test]
    fn wrong_top_level_shape_is_treated_as_corrupt() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::write(paths.presets_file(), r#"{"presets": 5}"#).unwrap();
        let loaded = load_all(&paths);
        assert!(loaded.presets.iter().all(|p| p.builtin));
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(quarantined_files(&paths.data_dir, "presets.json").len(), 1);
    }

    #[test]
    fn settings_values_are_clamped_on_load() {
        let (_dir, paths) = temp_paths();
        let mut settings = Settings::default();
        settings.detection_interval_ms = 10;
        settings.preview_zoom = 100.0;
        save_settings(&paths, &settings).unwrap();

        let loaded = load_all(&paths);
        assert_eq!(loaded.settings.detection_interval_ms, MIN_DETECTION_INTERVAL_MS);
        assert_eq!(loaded.settings.preview_zoom, MAX_PREVIEW_ZOOM);

        settings.detection_interval_ms = 999_999;
        settings.preview_zoom = 0.1;
        save_settings(&paths, &settings).unwrap();
        let loaded = load_all(&paths);
        assert_eq!(loaded.settings.detection_interval_ms, MAX_DETECTION_INTERVAL_MS);
        assert_eq!(loaded.settings.preview_zoom, MIN_PREVIEW_ZOOM);
    }

    #[test]
    fn non_finite_zoom_falls_back_to_default() {
        let mut settings = Settings { preview_zoom: f32::NAN, ..Settings::default() };
        sanitize_settings(&mut settings);
        assert_eq!(settings.preview_zoom, DEFAULT_PREVIEW_ZOOM);
        settings.preview_zoom = f32::INFINITY;
        sanitize_settings(&mut settings);
        assert_eq!(settings.preview_zoom, DEFAULT_PREVIEW_ZOOM);
    }

    #[test]
    fn recent_ids_are_deduplicated_and_capped() {
        let mut settings = Settings {
            recent_preset_ids: (0..12).map(|i| format!("p{}", i % 10)).collect(),
            ..Settings::default()
        };
        sanitize_settings(&mut settings);
        assert_eq!(settings.recent_preset_ids.len(), MAX_RECENT_PRESETS);
        assert_eq!(settings.recent_preset_ids[0], "p0");
        let unique: HashSet<_> = settings.recent_preset_ids.iter().collect();
        assert_eq!(unique.len(), settings.recent_preset_ids.len());
    }

    #[test]
    fn migrate_unversioned_settings_to_current() {
        let migrated = migrate_settings(json!({"preview_zoom": 6.0}));
        assert_eq!(migrated["version"], json!(SETTINGS_VERSION));
        assert_eq!(migrated["preview_zoom"], json!(6.0));
    }

    #[test]
    fn migrate_current_settings_is_noop() {
        let input = json!({"version": SETTINGS_VERSION, "detection_interval_ms": 500});
        assert_eq!(migrate_settings(input.clone()), input);
    }

    #[test]
    fn newer_settings_version_warns_and_loads_best_effort() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            paths.settings_file(),
            json!({"version": 99, "detection_interval_ms": 500, "future_field": true}).to_string(),
        )
        .unwrap();
        let loaded = load_all(&paths);
        assert_eq!(loaded.settings.detection_interval_ms, 500);
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("newer version"));
    }

    #[test]
    fn save_presets_skips_builtins_and_load_restores_flags() {
        let (_dir, paths) = temp_paths();
        let mut builtin = user_preset("builtin-test", "Built in");
        builtin.builtin = true;
        let mut custom = user_preset("custom-1", "Mine");
        custom.crosshair.color = Rgba::rgb(1, 2, 3);
        save_presets(&paths, &[builtin, custom.clone()]).unwrap();

        let raw: Value = serde_json::from_slice(&fs::read(paths.presets_file()).unwrap()).unwrap();
        let stored = raw["presets"].as_array().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0]["id"], json!("custom-1"));

        let loaded = load_all(&paths);
        let builtins = builtin_presets().len();
        assert_eq!(loaded.presets.len(), builtins + 1);
        assert!(loaded.presets[..builtins].iter().all(|p| p.builtin));
        let last = loaded.presets.last().unwrap();
        assert!(!last.builtin);
        assert_eq!(last.name, custom.name);
        assert_eq!(last.crosshair, custom.crosshair);
    }

    #[test]
    fn duplicate_user_preset_ids_get_new_ids() {
        let (_dir, paths) = temp_paths();
        save_presets(&paths, &[user_preset("same", "A"), user_preset("same", "B")]).unwrap();
        let loaded = load_all(&paths);
        let user: Vec<_> = loaded.presets.iter().filter(|p| !p.builtin).collect();
        assert_eq!(user.len(), 2);
        assert_ne!(user[0].id, user[1].id);
        assert_eq!(user[0].id, "same");
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn profiles_round_trip() {
        let (_dir, paths) = temp_paths();
        let profile = GameProfile { id: "pr1".into(), name: "CS2".into(), exe: "cs2.exe".into(), preset_id: Some("x".into()), enabled: false };
        save_profiles(&paths, std::slice::from_ref(&profile)).unwrap();
        let loaded = load_all(&paths);
        assert_eq!(loaded.profiles, vec![profile]);
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn write_atomic_replaces_content_and_leaves_no_temp_files() {
        let (_dir, paths) = temp_paths();
        let target = paths.data_dir.join("file.json");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
        let entries: Vec<_> = fs::read_dir(&paths.data_dir).unwrap().collect();
        assert_eq!(entries.len(), 1, "unexpected leftovers: {entries:?}");
    }

    #[test]
    fn save_worker_coalesces_and_writes_latest_on_drop() {
        let (_dir, paths) = temp_paths();
        let worker = SaveWorker::spawn(paths.clone());
        for i in 0..50u32 {
            let settings = Settings { detection_interval_ms: 250 + i, ..Settings::default() };
            worker.save(SaveJob { settings: Some(settings), ..SaveJob::default() });
        }
        let last = Settings { detection_interval_ms: 777, ..Settings::default() };
        worker.save(SaveJob { settings: Some(last), ..SaveJob::default() });
        drop(worker);

        let loaded = load_all(&paths);
        assert_eq!(loaded.settings.detection_interval_ms, 777);
    }

    #[test]
    fn save_worker_flush_makes_writes_visible() {
        let (_dir, paths) = temp_paths();
        let worker = SaveWorker::spawn(paths.clone());
        worker.save(SaveJob { profiles: Some(vec![GameProfile::default()]), ..SaveJob::default() });
        worker.flush();
        assert!(paths.profiles_file().exists());
        assert!(worker.take_errors().is_empty());
    }

    #[test]
    fn save_worker_reports_write_errors() {
        let (dir, paths) = temp_paths();
        // Make the data directory a regular file so writes below it fail.
        fs::write(dir.path().join("data"), "blocker").unwrap();
        let worker = SaveWorker::spawn(paths);
        worker.save(SaveJob { presets: Some(vec![user_preset("a", "A")]), ..SaveJob::default() });
        worker.flush();
        let errors = worker.take_errors();
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert!(errors[0].contains("presets"));
        assert!(worker.take_errors().is_empty());
    }

    #[test]
    fn init_logging_creates_file_and_truncates_when_oversized() {
        let (_dir, paths) = temp_paths();
        fs::create_dir_all(&paths.log_dir).unwrap();
        let big = vec![b'x'; (crate::storage::MAX_LOG_BYTES + 1) as usize];
        fs::write(paths.log_dir.join(crate::storage::LOG_FILE_NAME), big).unwrap();

        let path = crate::storage::init_logging(&paths).expect("init logging");
        assert!(path.exists());
        assert_eq!(fs::metadata(&path).unwrap().len(), 0);
        tracing::info!("logging test line");
    }
}
