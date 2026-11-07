use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use aurora_asset::AssetError;
use aurora_asset::fsd::FsdError;

/// A file of a load that could not be used, with why.
#[derive(Debug)]
pub struct FileProblem {
    pub path: PathBuf,
    pub problem: LoadProblem,
}

#[derive(Debug)]
pub enum LoadProblem {
    NotFound,
    /// A dropped folder.
    Folder,
    Unrecognised,
    Read(io::Error),
    Asset(AssetError),
    Fsd(FsdError),
    /// A valid file without a single image.
    NoAssets,
}

/// The sentence the load-problems dialog shows after the file name.
impl fmt::Display for LoadProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadProblem::NotFound => f.write_str("file not found"),
            LoadProblem::Folder => f.write_str("this is a folder, drop the files in it instead"),
            LoadProblem::Unrecognised => f.write_str("not an Aurora .asset or FSD .assets file"),
            LoadProblem::Read(error) => write!(f, "the file could not be read: {error}"),
            LoadProblem::Asset(error) => error.fmt(f),
            LoadProblem::Fsd(error) => error.fmt(f),
            LoadProblem::NoAssets => f.write_str("doesn't contain any assets"),
        }
    }
}

impl Error for LoadProblem {}
