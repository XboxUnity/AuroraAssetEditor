use aurora_console::Timeouts;
use aurora_online::OnlineClient;

use super::{ConsoleWorker, Job, JobOutcome, ProgressSink, ProgressSlot};

/// Runs jobs on a worker thread; each worker thread owns one.
///
/// A `ConsoleClient` takes its status callback once, when it is built, so the callback forwards into
/// `running_job_progress`, which `execute` points at the sink of the job it runs.
pub struct JobWorkers {
    console: ConsoleWorker,
    online: OnlineClient,
    running_job_progress: ProgressSlot,
}

impl JobWorkers {
    pub fn new(timeouts: Timeouts, online: OnlineClient) -> JobWorkers {
        let running_job_progress = ProgressSlot::new();
        JobWorkers {
            console: ConsoleWorker::new(timeouts, running_job_progress.clone()),
            online,
            running_job_progress,
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
            Job::Console(request) => JobOutcome::Console(self.console.run(request, &self.online)),
        }
    }
}

/// Points `running_job_progress` at one job's sink, and clears it when the job ends, even by a panic.
struct ProgressForwarding {
    slot: ProgressSlot,
}

impl ProgressForwarding {
    fn start(slot: &ProgressSlot, progress: &ProgressSink) -> ProgressForwarding {
        slot.set(Some(ProgressSink::clone(progress)));
        ProgressForwarding { slot: slot.clone() }
    }
}

impl Drop for ProgressForwarding {
    fn drop(&mut self) {
        self.slot.set(None);
    }
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
