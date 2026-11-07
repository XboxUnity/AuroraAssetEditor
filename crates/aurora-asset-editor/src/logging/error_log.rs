use super::error_chain::error_chain;
use chrono::Local;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const ERROR_LOG_FILE: &str = "error.log";

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S %:z";
const CAUSE_PREFIX: &str = "  caused by: ";

/// Appends entries to `error.log`. Clones share one lock, so entries from different threads never interleave.
#[derive(Clone)]
pub struct ErrorLog {
    path: PathBuf,
    write_lock: Arc<Mutex<()>>,
}

impl ErrorLog {
    pub fn in_dir(config_dir: &Path) -> ErrorLog {
        ErrorLog {
            path: config_dir.join(ERROR_LOG_FILE),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(&self, context: &str, error: &(dyn Error + 'static)) -> io::Result<()> {
        let mut messages = error_chain(error).into_iter();
        let outermost = messages.next().unwrap_or_default();
        let mut body = format!("{context}: {outermost}\n");
        for cause in messages {
            body.push_str(CAUSE_PREFIX);
            body.push_str(&cause);
            body.push('\n');
        }
        self.append(&body)
    }

    pub fn record_message(&self, context: &str, message: &str) -> io::Result<()> {
        self.append(&format!("{context}: {message}\n"))
    }

    fn append(&self, body: &str) -> io::Result<()> {
        let entry = format!("[{}] {body}\n", Local::now().format(TIMESTAMP_FORMAT));
        let _held = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(entry.as_bytes())
    }
}
