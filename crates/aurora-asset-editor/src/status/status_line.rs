pub const READY_TEXT: &str = "Ready";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Error,
}

/// The line at the bottom of the window: the last result, and the running job's progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    pub text: String,
    pub severity: Severity,
    pub progress: Option<String>,
}

impl StatusLine {
    pub fn ready() -> StatusLine {
        StatusLine::info(READY_TEXT)
    }

    pub fn info(text: impl Into<String>) -> StatusLine {
        StatusLine {
            text: text.into(),
            severity: Severity::Info,
            progress: None,
        }
    }

    pub fn error(text: impl Into<String>) -> StatusLine {
        StatusLine {
            text: text.into(),
            severity: Severity::Error,
            progress: None,
        }
    }
}
