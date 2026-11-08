use async_channel::{Receiver, TryRecvError};

use crate::jobs::JobOutcome;

use super::App;

impl App {
    /// The outcomes of jobs that ended after the window closed, applied as usual so every failure among them
    /// is written to `error.log`. Nothing is on screen any more to show them.
    pub fn apply_outcomes_after_close(&mut self, outcomes: &Receiver<JobOutcome>) {
        loop {
            match outcomes.try_recv() {
                // A picture resized for the closed window has nowhere to go.
                Ok(JobOutcome::Preview(_)) => {}
                Ok(outcome) => self.on_job_outcome(outcome),
                Err(TryRecvError::Empty | TryRecvError::Closed) => return,
            }
        }
    }
}
