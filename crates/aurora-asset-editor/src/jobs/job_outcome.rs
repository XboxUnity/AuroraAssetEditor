use aurora_console::ConsoleStatus;

use crate::files::LoadedFiles;

use super::{ConsoleOutcome, ImportResult};

/// What a job hands back to the UI thread: its result, or progress while it runs.
#[derive(Debug)]
pub enum JobOutcome {
    Loaded(LoadedFiles),
    Imported(Vec<ImportResult>),
    Console(ConsoleOutcome),
    Progress(Progress),
}

/// What a running job reports before it is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Console(ConsoleStatus),
}
