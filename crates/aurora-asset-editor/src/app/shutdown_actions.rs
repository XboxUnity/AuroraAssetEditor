use async_channel::{Receiver, TryRecvError};

use crate::jobs::JobOutcome;

use super::App;

impl App {
    /// The outcomes of jobs that ended after the window closed, applied as usual so every failure among them
    /// is written to `error.log`. Nothing is on screen any more to show them.
    pub fn apply_outcomes_after_close(&mut self, outcomes: &Receiver<JobOutcome>) {
        loop {
            match outcomes.try_recv() {
                Ok(JobOutcome::Preview(resized)) => log::debug!(
                    "A preview of {:?} was resized after the window closed",
                    resized.picture
                ),
                Ok(outcome) => self.on_job_outcome(outcome),
                Err(TryRecvError::Empty | TryRecvError::Closed) => return,
            }
        }
    }
}
