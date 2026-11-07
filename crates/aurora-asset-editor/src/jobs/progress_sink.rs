use std::sync::Arc;

use super::JobOutcome;

/// Where a running job sends its progress outcomes. It is called on the worker thread.
pub type ProgressSink = Arc<dyn Fn(JobOutcome) + Send + Sync>;
