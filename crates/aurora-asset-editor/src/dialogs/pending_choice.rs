use std::path::PathBuf;

/// What a choice dialog is about, so its answer can carry on with the files that were dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingChoice {
    /// Asked for one dropped file at a time; `remaining` waits for its turn.
    IconOrBanner {
        path: PathBuf,
        remaining: Vec<PathBuf>,
    },
    /// Asked once for the first dropped file; `rest` follows its answer.
    ReplaceScreenshot { first: PathBuf, rest: Vec<PathBuf> },
}
