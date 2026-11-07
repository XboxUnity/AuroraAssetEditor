use std::collections::VecDeque;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use async_channel::{SendError, Sender};

use super::{Job, JobExecutor, JobOutcome, JobPriority, JobRunner, ProgressSink};

const LOAD_THREAD: &str = "load and import";
// One thread, so the app holds one FTP session at a time: Aurora allows few sessions and serves them slowly.
const CONSOLE_THREAD: &str = "console";
const ONLINE_THREAD: &str = "online";

/// Runs jobs on three worker threads: loading and importing files, the console, and online searches. Each
/// thread takes its `User` jobs before its `Background` ones; outcomes and progress go to `outcomes`.
pub struct ThreadRunner {
    queues: Arc<RunnerQueues>,
    threads: Vec<JoinHandle<()>>,
}

impl ThreadRunner {
    /// Each thread gets its own executor from `make_executor`.
    pub fn new<E: JobExecutor>(
        make_executor: impl Fn() -> E,
        outcomes: Sender<JobOutcome>,
    ) -> io::Result<ThreadRunner> {
        let queues = Arc::new(RunnerQueues::default());
        let mut runner = ThreadRunner {
            queues: Arc::clone(&queues),
            threads: Vec::new(),
        };
        let threads = [
            (LOAD_THREAD, QueueId::Load),
            (CONSOLE_THREAD, QueueId::Console),
            (ONLINE_THREAD, QueueId::Online),
        ];
        for (name, queue_id) in threads {
            let executor = make_executor();
            let queues = Arc::clone(&queues);
            let outcomes = outcomes.clone();
            let thread = thread::Builder::new()
                .name(format!("{name} worker"))
                .spawn(move || work(executor, queues.queue(queue_id), &outcomes))?;
            runner.threads.push(thread);
        }
        Ok(runner)
    }

    /// A handle the app submits jobs through, while this runner stays with `main` for the shutdown.
    pub fn submitter(&self) -> JobSubmitter {
        JobSubmitter {
            queues: Arc::clone(&self.queues),
        }
    }

    /// Drops the jobs that have not started, waits for the running ones to end and joins the threads.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.queues.close_all();
        for thread in self.threads.drain(..) {
            let name = thread.thread().name().unwrap_or_default().to_owned();
            if thread.join().is_err() {
                log::error!("The {name} thread ended with a panic");
            }
        }
    }
}

impl JobRunner for ThreadRunner {
    fn submit(&mut self, job: Job, priority: JobPriority) {
        self.queues.for_job(&job).push(job, priority);
    }
}

impl Drop for ThreadRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Submits jobs to a `ThreadRunner`'s threads.
#[derive(Clone)]
pub struct JobSubmitter {
    queues: Arc<RunnerQueues>,
}

impl JobRunner for JobSubmitter {
    fn submit(&mut self, job: Job, priority: JobPriority) {
        self.queues.for_job(&job).push(job, priority);
    }
}

fn work<E: JobExecutor>(mut executor: E, queue: &JobQueue, outcomes: &Sender<JobOutcome>) {
    let progress: ProgressSink = {
        let outcomes = outcomes.clone();
        Arc::new(move |outcome| send(&outcomes, outcome))
    };
    while let Some(job) = queue.next() {
        // The panic hook has logged the panic; the thread goes on with the next job.
        match panic::catch_unwind(AssertUnwindSafe(|| executor.execute(job, &progress))) {
            Ok(outcome) => send(outcomes, outcome),
            Err(_panic) => log::error!("A job on the {} thread panicked", thread_name()),
        }
    }
}

fn send(outcomes: &Sender<JobOutcome>, outcome: JobOutcome) {
    // Sending fails only once the receiver is gone, which happens when the event loop has ended.
    if let Err(SendError(_unreceived)) = outcomes.send_blocking(outcome) {
        log::debug!("A job outcome arrived after the window closed");
    }
}

fn thread_name() -> String {
    thread::current().name().unwrap_or_default().to_owned()
}

#[derive(Debug, Clone, Copy)]
enum QueueId {
    Load,
    Console,
    Online,
}

#[derive(Default)]
struct RunnerQueues {
    load: JobQueue,
    console: JobQueue,
    online: JobQueue,
}

impl RunnerQueues {
    fn queue(&self, id: QueueId) -> &JobQueue {
        match id {
            QueueId::Load => &self.load,
            QueueId::Console => &self.console,
            QueueId::Online => &self.online,
        }
    }

    fn for_job(&self, job: &Job) -> &JobQueue {
        match job {
            Job::Load(_) | Job::Import(_) => &self.load,
            Job::Console(_) => &self.console,
            Job::Online(_) => &self.online,
        }
    }

    fn close_all(&self) {
        for queue in [&self.load, &self.console, &self.online] {
            queue.close();
        }
    }
}

/// One thread's waiting jobs.
#[derive(Default)]
struct JobQueue {
    state: Mutex<QueueState>,
    changed: Condvar,
}

#[derive(Default)]
struct QueueState {
    user: VecDeque<Job>,
    background: VecDeque<Job>,
    closed: bool,
}

impl JobQueue {
    fn push(&self, job: Job, priority: JobPriority) {
        let mut state = self.lock();
        if state.closed {
            log::debug!("A job was submitted after the shutdown began and does not run");
            return;
        }
        match priority {
            JobPriority::User => state.user.push_back(job),
            JobPriority::Background => state.background.push_back(job),
        }
        self.changed.notify_one();
    }

    /// The next job, `User` ones first; `None` once the queue is closed.
    fn next(&self) -> Option<Job> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(job) = state
                .user
                .pop_front()
                .or_else(|| state.background.pop_front())
            {
                return Some(job);
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.user.clear();
        state.background.clear();
        self.changed.notify_all();
    }

    // Nothing panics while holding the lock, so a poisoned lock still holds consistent queues.
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
