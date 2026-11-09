use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub const VERSION_FLAG: &str = "--version";
pub const SMOKE_TEST_FLAG: &str = "--smoke-test";

/// What the command line asks the app to do. A flag counts only as the first argument; anywhere else it is a
/// file name like any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    Version,
    /// The editor, with these files loaded.
    Window {
        paths: Vec<PathBuf>,
    },
    /// Loads `asset` and saves it to `output` through the real window, then exits.
    SmokeTest {
        asset: PathBuf,
        output: PathBuf,
    },
}

impl LaunchMode {
    pub fn from_arguments(arguments: Vec<OsString>) -> Result<LaunchMode, LaunchModeError> {
        if arguments == [VERSION_FLAG] {
            return Ok(LaunchMode::Version);
        }
        if arguments
            .first()
            .is_some_and(|first| first == SMOKE_TEST_FLAG)
        {
            return match <[OsString; 3]>::try_from(arguments) {
                Ok([_, asset, output]) => Ok(LaunchMode::SmokeTest {
                    asset: asset.into(),
                    output: output.into(),
                }),
                Err(_) => Err(LaunchModeError::SmokeTestPaths),
            };
        }
        Ok(LaunchMode::Window {
            paths: arguments.into_iter().map(PathBuf::from).collect(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchModeError {
    /// `--smoke-test` came without exactly two paths after it.
    SmokeTestPaths,
}

impl fmt::Display for LaunchModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchModeError::SmokeTestPaths => write!(
                f,
                "{SMOKE_TEST_FLAG} needs an asset file and an output file"
            ),
        }
    }
}

impl Error for LaunchModeError {}
