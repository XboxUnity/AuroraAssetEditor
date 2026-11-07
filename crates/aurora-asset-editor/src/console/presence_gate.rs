use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Tells queued presence checks, on the worker thread, whether they are still wanted: not after a newer
/// refresh, and not after a check of the same refresh found the console unreachable. Clones share the state.
#[derive(Debug, Clone, Default)]
pub struct PresenceGate {
    state: Arc<Mutex<GateState>>,
}

#[derive(Debug, Default)]
struct GateState {
    generation: u64,
    /// The refresh whose checks are skipped because the console could not be reached.
    unreachable_generation: Option<u64>,
}

impl PresenceGate {
    pub fn current(&self) -> u64 {
        self.state().generation
    }

    pub(crate) fn start_refresh(&self) -> u64 {
        let mut state = self.state();
        state.generation += 1;
        state.generation
    }

    pub fn should_skip(&self, generation: u64) -> bool {
        let state = self.state();
        generation != state.generation || state.unreachable_generation == Some(generation)
    }

    pub fn mark_unreachable(&self, generation: u64) {
        self.state().unreachable_generation = Some(generation);
    }

    /// A transfer that succeeded shows the console answers, so checks may run again.
    pub(crate) fn mark_reachable(&self) {
        self.state().unreachable_generation = None;
    }

    fn state(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
