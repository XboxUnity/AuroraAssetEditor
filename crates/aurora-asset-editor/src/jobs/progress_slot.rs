use std::sync::{Arc, Mutex, PoisonError};

use super::{JobOutcome, ProgressSink};

/// Where the status callback of a long-lived client sends progress: to the sink of the job that is running
/// now, or nowhere between jobs. Clones share the slot.
#[derive(Clone, Default)]
pub struct ProgressSlot {
    sink: Arc<Mutex<Option<ProgressSink>>>,
}

impl ProgressSlot {
    pub fn new() -> ProgressSlot {
        ProgressSlot::default()
    }

    pub fn set(&self, sink: Option<ProgressSink>) {
        *self.sink.lock().unwrap_or_else(PoisonError::into_inner) = sink;
    }

    /// Sends `outcome` to the current sink, if there is one.
    pub fn send(&self, outcome: JobOutcome) {
        // The sink may wait for the UI thread, so the lock is released before it is called.
        let sink = self
            .sink
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(sink) = sink {
            sink(outcome);
        }
    }
}
