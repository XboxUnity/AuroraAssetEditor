use super::{Job, JobOutcome, JobWorkers, ProgressSink};

/// Runs one job at a time on a worker thread; `ThreadRunner` gives each of its threads its own.
pub trait JobExecutor: Send + 'static {
    fn execute(&mut self, job: Job, progress: &ProgressSink) -> JobOutcome;
}

impl JobExecutor for JobWorkers {
    fn execute(&mut self, job: Job, progress: &ProgressSink) -> JobOutcome {
        JobWorkers::execute(self, job, progress)
    }
}
