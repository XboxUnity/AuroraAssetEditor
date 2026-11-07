use aurora_console::ConsoleStatus;

use crate::files::LoadedFiles;

use super::{ConsoleJobKind, ConsoleOutcome, ImportResult, OnlineOutcome};

/// What a job hands back to the UI thread: its result, or progress while it runs.
#[derive(Debug)]
pub enum JobOutcome {
    Loaded(LoadedFiles),
    Imported(Vec<ImportResult>),
    Console(ConsoleOutcome),
    Online(OnlineOutcome),
    Progress(Progress),
}

/// What a running job reports before it is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Console {
        kind: ConsoleJobKind,
        status: ConsoleStatus,
    },
    /// A bulk cover run is about to handle a game.
    Bulk {
        /// Counts from 1.
        position: usize,
        total: usize,
        title: String,
    },
}
