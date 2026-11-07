use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use aurora_asset_editor::jobs::{Job, JobPriority, JobRunner};

#[derive(Default)]
struct Queues {
    user: VecDeque<Job>,
    background: VecDeque<Job>,
}

/// Keeps submitted jobs until the test runs them. Clones share one queue.
#[derive(Clone, Default)]
pub struct QueuedRunner {
    queues: Rc<RefCell<Queues>>,
}

impl QueuedRunner {
    pub fn new() -> QueuedRunner {
        QueuedRunner::default()
    }

    /// The job a worker would take next: the oldest `User` job, else the oldest `Background` one.
    pub fn next_job(&self) -> Option<Job> {
        let mut queues = self.queues.borrow_mut();
        queues
            .user
            .pop_front()
            .or_else(|| queues.background.pop_front())
    }

    pub fn queued(&self) -> usize {
        let queues = self.queues.borrow();
        queues.user.len() + queues.background.len()
    }
}

impl JobRunner for QueuedRunner {
    fn submit(&mut self, job: Job, priority: JobPriority) {
        let mut queues = self.queues.borrow_mut();
        match priority {
            JobPriority::User => queues.user.push_back(job),
            JobPriority::Background => queues.background.push_back(job),
        }
    }
}
