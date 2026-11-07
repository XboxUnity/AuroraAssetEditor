use std::sync::{Arc, Mutex, PoisonError};

use aurora_console::Timeouts;
use aurora_online::OnlineClient;

use super::{Job, JobOutcome, ProgressSink};

/// Runs jobs on a worker thread; each worker thread owns one.
///
/// A `ConsoleClient` takes its status callback once, when it is built, so the callback forwards into
/// `running_job_progress`, which `execute` points at the sink of the job it runs.
pub struct JobWorkers {
    #[expect(dead_code, reason = "read once console jobs exist")]
    timeouts: Timeouts,
    #[expect(dead_code, reason = "read once online jobs exist")]
    online: OnlineClient,
    running_job_progress: Arc<Mutex<Option<ProgressSink>>>,
}

impl JobWorkers {
    pub fn new(timeouts: Timeouts, online: OnlineClient) -> JobWorkers {
        JobWorkers {
            timeouts,
            online,
            running_job_progress: Arc::new(Mutex::new(None)),
        }
    }

    pub fn execute(&mut self, job: Job, progress: &ProgressSink) -> JobOutcome {
        let _forwarding = ProgressForwarding::start(&self.running_job_progress, progress);
        self.run(job)
    }

    fn run(&mut self, job: Job) -> JobOutcome {
        match job {
            Job::Load(load) => JobOutcome::Loaded(load.run()),
            Job::Import(import) => JobOutcome::Imported(import.run()),
        }
    }
}

/// Points `running_job_progress` at one job's sink, and clears it when the job ends, even by a panic.
struct ProgressForwarding {
    slot: Arc<Mutex<Option<ProgressSink>>>,
}

impl ProgressForwarding {
    fn start(
        slot: &Arc<Mutex<Option<ProgressSink>>>,
        progress: &ProgressSink,
    ) -> ProgressForwarding {
        set_slot(slot, Some(Arc::clone(progress)));
        ProgressForwarding {
            slot: Arc::clone(slot),
        }
    }
}

impl Drop for ProgressForwarding {
    fn drop(&mut self) {
        set_slot(&self.slot, None);
    }
}

fn set_slot(slot: &Mutex<Option<ProgressSink>>, progress: Option<ProgressSink>) {
    *slot.lock().unwrap_or_else(PoisonError::into_inner) = progress;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_workers_can_move_to_a_worker_thread() {
        fn assert_send<T: Send + 'static>() {}
        assert_send::<JobWorkers>();
        assert_send::<Job>();
        assert_send::<JobOutcome>();
    }
}
