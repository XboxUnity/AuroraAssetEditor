use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tells queued presence checks, on the worker thread, whether they are still wanted: not after a newer
/// refresh, and not after a check of the same refresh found the console unreachable. Clones share the state.
#[derive(Debug, Clone, Default)]
pub struct PresenceGate {
    state: Arc<GateState>,
}

#[derive(Debug, Default)]
struct GateState {
    generation: AtomicU64,
    unreachable_generation: AtomicU64,
}

impl PresenceGate {
    pub fn current(&self) -> u64 {
        self.state.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn start_refresh(&self) -> u64 {
        self.state.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn should_skip(&self, generation: u64) -> bool {
        generation != self.current()
            || generation == self.state.unreachable_generation.load(Ordering::SeqCst)
    }

    pub fn mark_unreachable(&self, generation: u64) {
        self.state
            .unreachable_generation
            .store(generation, Ordering::SeqCst);
    }

    /// A transfer that succeeded shows the console answers, so checks may run again. Refresh generations
    /// start at 1, so 0 matches no check that was queued by a refresh.
    pub(crate) fn mark_reachable(&self) {
        self.state.unreachable_generation.store(0, Ordering::SeqCst);
    }
}
