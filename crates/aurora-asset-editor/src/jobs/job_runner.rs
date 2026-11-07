use super::{Job, JobPriority};

/// Queues jobs for the workers. Outcomes come back to the app through `App::on_job_outcome`.
pub trait JobRunner {
    fn submit(&mut self, job: Job, priority: JobPriority);
}
