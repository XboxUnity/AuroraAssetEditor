use std::path::PathBuf;

use crate::editor::{DecodedKind, PreparedImage};

use super::{FileProblem, ImageLoadError};

/// A file read, parsed and decoded on a worker, ready to be put on the pages.
#[derive(Debug)]
pub enum LoadedFile {
    Asset {
        path: PathBuf,
        decoded: Box<DecodedKind>,
    },
    Fsd {
        path: PathBuf,
        images: Box<FsdImages>,
    },
}

/// The images of an FSD file, decoded but not resized.
#[derive(Debug, Default)]
pub struct FsdImages {
    pub cover: Option<PreparedImage>,
    pub background: Option<PreparedImage>,
    pub icon: Option<PreparedImage>,
    pub banner: Option<PreparedImage>,
    pub screenshots: Vec<PreparedImage>,
    /// Images that could not be decoded, by the name a dialog gives them.
    pub unreadable: Vec<(String, ImageLoadError)>,
}

/// The result of one load job: what could be used and what could not, in the order of the paths.
#[derive(Debug, Default)]
pub struct LoadedFiles {
    pub files: Vec<LoadedFile>,
    pub problems: Vec<FileProblem>,
}
