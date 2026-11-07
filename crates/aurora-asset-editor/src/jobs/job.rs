use super::LoadJob;

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
}
