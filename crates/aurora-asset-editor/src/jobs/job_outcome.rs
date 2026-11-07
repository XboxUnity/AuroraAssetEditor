use crate::files::LoadedFiles;

use super::ImportResult;

/// What a job hands back to the UI thread: its result, or progress while it runs.
#[derive(Debug)]
pub enum JobOutcome {
    Loaded(LoadedFiles),
    Imported(Vec<ImportResult>),
}
