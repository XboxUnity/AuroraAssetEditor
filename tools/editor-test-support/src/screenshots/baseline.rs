use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::image_files::{png_names, png_path};
use super::run_folder::IMAGES_FOLDER;
use super::run_manifest::{MANIFEST_FILE, ManifestError, RunManifest};

/// An earlier finished run of this platform that the current run is compared with: a folder
/// holding `manifest.txt` and `images/<image>.png`. Nothing else in it is read.
#[derive(Debug, Clone)]
pub struct Baseline {
    folder: PathBuf,
    manifest: RunManifest,
}

impl Baseline {
    /// Refuses a folder that is not a finished, unfiltered run of `platform`.
    pub fn open(folder: &Path, platform: &str) -> Result<Baseline, BaselineError> {
        if !folder.is_dir() {
            return Err(BaselineError::NotAFolder(folder.to_path_buf()));
        }
        let manifest = RunManifest::read(folder).map_err(|error| BaselineError::Manifest {
            folder: folder.to_path_buf(),
            error,
        })?;
        if manifest.platform != platform {
            return Err(BaselineError::OtherPlatform {
                folder: folder.to_path_buf(),
                found: manifest.platform,
                expected: platform.to_owned(),
            });
        }
        if let Some(filter) = manifest.filter {
            return Err(BaselineError::Filtered {
                folder: folder.to_path_buf(),
                filter,
            });
        }
        Ok(Baseline {
            folder: folder.to_path_buf(),
            manifest,
        })
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn manifest(&self) -> &RunManifest {
        &self.manifest
    }

    pub fn image_path(&self, image: &str) -> PathBuf {
        png_path(&self.folder.join(IMAGES_FOLDER), image)
    }

    pub fn image_names(&self) -> io::Result<BTreeSet<String>> {
        png_names(&self.folder.join(IMAGES_FOLDER))
    }
}

#[derive(Debug)]
pub enum BaselineError {
    NotAFolder(PathBuf),
    Manifest {
        folder: PathBuf,
        error: ManifestError,
    },
    OtherPlatform {
        folder: PathBuf,
        found: String,
        expected: String,
    },
    Filtered {
        folder: PathBuf,
        filter: String,
    },
}

impl fmt::Display for BaselineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFolder(path) => {
                write!(formatter, "The baseline {} is not a folder", path.display())
            }
            Self::Manifest { folder, error } => write!(
                formatter,
                "The baseline {} has no usable {MANIFEST_FILE}: {error}",
                folder.display()
            ),
            Self::OtherPlatform {
                folder,
                found,
                expected,
            } => write!(
                formatter,
                "The baseline {} was rendered on {found}; this run renders {expected}, and screenshots are only compared on the platform that rendered them",
                folder.display()
            ),
            Self::Filtered { folder, filter } => write!(
                formatter,
                "The baseline {} was a filtered run (\"{filter}\"), so it holds only some of the screens",
                folder.display()
            ),
        }
    }
}

impl Error for BaselineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Manifest { error, .. } => Some(error),
            _ => None,
        }
    }
}
