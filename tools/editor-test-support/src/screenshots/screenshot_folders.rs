use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::path_error::with_path;
use super::run_folder::{RunFolder, current_platform};
use super::run_manifest::{MANIFEST_FILE, ManifestError, RunManifest};

pub const PREVIOUS_FOLDER: &str = "previous";

/// The folders the screenshot runs of one platform write into: `<root>/<platform>` for the
/// current run and `<root>/previous/<platform>` for the last finished unfiltered local run, the
/// default baseline.
#[derive(Debug, Clone)]
pub struct ScreenshotFolders {
    root: PathBuf,
    platform: String,
}

impl ScreenshotFolders {
    pub fn new(root: PathBuf, platform: String) -> ScreenshotFolders {
        ScreenshotFolders { root, platform }
    }

    /// `target/ui-screenshots` of the workspace this crate belongs to, for this platform.
    pub fn for_workspace() -> ScreenshotFolders {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("this crate lives two folders below the workspace root");
        ScreenshotFolders::new(
            workspace.join("target").join("ui-screenshots"),
            current_platform(),
        )
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub fn current(&self) -> RunFolder {
        RunFolder::new(self.current_path())
    }

    pub fn previous(&self) -> PathBuf {
        self.root.join(PREVIOUS_FOLDER).join(&self.platform)
    }

    /// Makes room for a new run. An earlier finished unfiltered run in the current folder becomes
    /// the previous run; any other earlier run is deleted, and the returned note says why. The
    /// current folder is then created empty.
    pub fn start_run(&self) -> io::Result<Option<String>> {
        let current = self.current_path();
        let exists = current
            .try_exists()
            .map_err(|error| with_path(&current, error))?;
        let note = if exists { self.retire(&current)? } else { None };
        fs::create_dir_all(&current).map_err(|error| with_path(&current, error))?;
        Ok(note)
    }

    fn current_path(&self) -> PathBuf {
        self.root.join(&self.platform)
    }

    fn retire(&self, current: &Path) -> io::Result<Option<String>> {
        let reason = match RunManifest::read(current) {
            Ok(manifest) => match manifest.filter {
                None => {
                    self.keep_as_previous(current)?;
                    return Ok(None);
                }
                Some(filter) => format!("it was filtered to \"{filter}\""),
            },
            Err(ManifestError::Missing(_)) => {
                format!("it did not finish (it has no {MANIFEST_FILE})")
            }
            Err(error) => format!("its manifest is unreadable: {error}"),
        };
        remove_folder(current)?;
        Ok(Some(format!(
            "The earlier run in {} was deleted, not kept as the baseline: {reason}",
            current.display()
        )))
    }

    fn keep_as_previous(&self, current: &Path) -> io::Result<()> {
        let previous = self.previous();
        if previous
            .try_exists()
            .map_err(|error| with_path(&previous, error))?
        {
            remove_folder(&previous)?;
        }
        if let Some(parent) = previous.parent() {
            fs::create_dir_all(parent).map_err(|error| with_path(parent, error))?;
        }
        fs::rename(current, &previous).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Could not move {} to {}: {error}",
                    current.display(),
                    previous.display()
                ),
            )
        })
    }
}

fn remove_folder(folder: &Path) -> io::Result<()> {
    fs::remove_dir_all(folder).map_err(|error| with_path(folder, error))
}
