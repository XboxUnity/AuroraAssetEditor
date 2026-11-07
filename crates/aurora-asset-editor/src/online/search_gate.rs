use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The current search's generation, shared with the online worker so the downloads of a replaced search are
/// skipped before they fetch anything. Clones share the state.
#[derive(Debug, Clone, Default)]
pub struct SearchGate {
    generation: Arc<AtomicU64>,
}

impl SearchGate {
    pub fn current(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn start_search(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn is_superseded(&self, generation: u64) -> bool {
        generation != self.current()
    }
}
