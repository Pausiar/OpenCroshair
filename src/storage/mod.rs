pub mod config;
pub mod import_export;

use config::Paths;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const LOG_FILE_NAME: &str = "opencrosshair.log";
/// The log is truncated at startup once it grows beyond this size.
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// Installs a global tracing subscriber writing to `<log_dir>/opencrosshair.log`. No network.
/// Fails if a subscriber was already installed in this process.
pub fn init_logging(paths: &Paths) -> io::Result<PathBuf> {
    fs::create_dir_all(&paths.log_dir)?;
    let path = paths.log_dir.join(LOG_FILE_NAME);
    let oversized = fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false);

    let mut options = OpenOptions::new();
    options.create(true);
    if oversized {
        options.write(true).truncate(true);
    } else {
        options.append(true);
    }
    let file = options.open(&path)?;
    let shared = Arc::new(Mutex::new(file));

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_target(false)
        .with_writer(move || LogSink(Arc::clone(&shared)))
        .try_init()
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(path)
}

/// Writer handed to tracing; every write goes through one shared file handle.
struct LogSink(Arc<Mutex<File>>);

impl Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        file.flush()
    }
}
