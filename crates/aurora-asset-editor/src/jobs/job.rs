use super::{ConsoleRequest, ImportJob, LoadJob, OnlineJob, PreviewJob};

/// Which queue a job waits in: a worker takes every `User` job before any `Background` one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPriority {
    User,
    Background,
}

/// Work that runs on a worker thread; its result comes back as a `JobOutcome`.
#[derive(Debug)]
pub enum Job {
    Load(LoadJob),
    Import(ImportJob),
    Console(ConsoleRequest),
    Online(OnlineJob),
    /// Submitted by the view, which takes the outcome.
    Preview(PreviewJob),
}
