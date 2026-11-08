use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use super::manifest_error::ManifestError;
use super::run_manifest::MANIFEST_FILE;

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
