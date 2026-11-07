use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::transfer::MAX_CONTENT_DB_BYTES;
use super::{ConsoleClient, ConsoleError};
use crate::games::{Game, read_content_db};
use crate::paths::{CONTENT_DB_FILE, DATABASES_DIR};

const TEMP_FILE_PREFIX: &str = "AuroraAssetEditor-";
const TEMP_FILE_SUFFIX: &str = ".db";
/// The delete is retried while another process holds the file, blocking for at most
/// `(CLEANUP_ATTEMPTS - 1) * CLEANUP_DELAY` (about 0.9 s).
const CLEANUP_ATTEMPTS: u32 = 10;
const CLEANUP_DELAY: Duration = Duration::from_millis(100);

/// The console's games, plus the temp copy of `Content.db` if it could not be deleted.
#[derive(Debug)]
pub struct GameListing {
    pub games: Vec<Game>,
    pub cleanup_error: Option<TempCleanupError>,
}

#[derive(Debug)]
pub struct TempCleanupError {
    pub path: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for TempCleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "could not remove the temporary copy {}",
            self.path.display()
        )
    }
}

impl std::error::Error for TempCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl ConsoleClient {
    pub fn list_games(&mut self) -> Result<GameListing, ConsoleError> {
        self.list_games_in(&std::env::temp_dir())
    }

    /// Downloads `Content.db` into `temp_dir` and reads it. The copy is removed on every path; one
    /// that cannot be removed is reported in `GameListing::cleanup_error`.
    pub fn list_games_in(&mut self, temp_dir: &Path) -> Result<GameListing, ConsoleError> {
        let bytes = self
            .run_operation(|client| {
                client.download_listed(DATABASES_DIR, CONTENT_DB_FILE, MAX_CONTENT_DB_BYTES)
            })?
            .ok_or(ConsoleError::ContentDbMissing)?;
        let path = write_temp_copy(temp_dir, &bytes).map_err(ConsoleError::TempFile)?;

        let games = read_content_db(&path);
        let cleanup_error = remove_with_retries(&path, CLEANUP_ATTEMPTS, CLEANUP_DELAY)
            .err()
            .map(|source| TempCleanupError { path, source });
        match games {
            Ok(games) => Ok(GameListing {
                games,
                cleanup_error,
            }),
            Err(read_error) => {
                if let Some(leftover) = &cleanup_error {
                    log::warn!("{leftover}: {}", leftover.source);
                }
                Err(ConsoleError::ContentDb(read_error))
            }
        }
    }
}

/// Returns the path of a closed temp file that the caller must remove.
fn write_temp_copy(temp_dir: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let mut file = tempfile::Builder::new()
        .prefix(TEMP_FILE_PREFIX)
        .suffix(TEMP_FILE_SUFFIX)
        .tempfile_in(temp_dir)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.into_temp_path().keep().map_err(|error| error.error)
}

/// The file may stay locked for a moment after SQLite closes it, notably on Windows.
pub(crate) fn remove_with_retries(path: &Path, attempts: u32, delay: Duration) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) if attempt >= attempts => return Err(error),
            Err(_) => {
                attempt += 1;
                thread::sleep(delay);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn cleanup_error_names_the_file_and_chains_the_cause() {
        let error = TempCleanupError {
            path: PathBuf::from("temp").join("AuroraAssetEditor-1.db"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        };

        assert_eq!(
            error.to_string(),
            format!(
                "could not remove the temporary copy {}",
                error.path.display()
            )
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn remove_with_retries_treats_missing_as_removed() {
        let directory = tempfile::tempdir().expect("a temp dir is created");

        let result = remove_with_retries(
            &directory.path().join("absent.db"),
            3,
            Duration::from_millis(1),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn remove_with_retries_deletes_an_existing_file() {
        let directory = tempfile::tempdir().expect("a temp dir is created");
        let path = directory.path().join("present.db");
        std::fs::write(&path, b"x").expect("the file is written");

        remove_with_retries(&path, 3, Duration::from_millis(1)).expect("the file is removed");

        assert!(!path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn cleanup_gives_up_after_bounded_attempts() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = tempfile::tempdir().expect("a temp dir is created");
        let path = directory.path().join("locked.db");
        let _exclusive = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0)
            .open(&path)
            .expect("the file opens exclusively");
        let delay = Duration::from_millis(50);

        let started = Instant::now();
        let result = remove_with_retries(&path, 3, delay);

        assert!(result.is_err());
        assert!(started.elapsed() >= delay * 2);
    }
}
