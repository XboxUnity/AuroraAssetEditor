use std::collections::VecDeque;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use async_channel::{SendError, Sender};

use super::{
    FailedJob, Job, JobExecutor, JobOutcome, JobPanic, JobPriority, JobRunner, ProgressSink,
};

const LOAD_THREAD: &str = "load and import";
// One thread, so the app holds one FTP session at a time: Aurora allows few sessions and serves them slowly.
const CONSOLE_THREAD: &str = "console";
const ONLINE_THREAD: &str = "online";

/// Runs jobs on three worker threads: loading and importing files, the console, and online searches. Each
/// thread takes its `User` jobs before its `Background` ones; outcomes and progress go to `outcomes`.
pub struct ThreadRunner {
    queues: Arc<RunnerQueues>,
    /// Waited for at the shutdown: an interrupted console upload would leave a half-written file behind.
    joined_threads: Vec<JoinHandle<()>>,
    /// Not waited for: an online fetch can take until its timeout, and nothing is lost when it is cut off.
    online_thread: Option<JoinHandle<()>>,
}

impl ThreadRunner {
    /// Each thread gets its own executor from `make_executor`, and a fresh one after a job panics.
    pub fn new<E: JobExecutor>(
        make_executor: impl Fn() -> E + Send + Sync + 'static,
        outcomes: Sender<JobOutcome>,
    ) -> io::Result<ThreadRunner> {
        let make_executor: Arc<dyn Fn() -> E + Send + Sync> = Arc::new(make_executor);
        let queues = Arc::new(RunnerQueues::default());
        let mut runner = ThreadRunner {
            queues: Arc::clone(&queues),
            joined_threads: Vec::new(),
            online_thread: None,
        };
        let threads = [
            (LOAD_THREAD, QueueId::Load),
            (CONSOLE_THREAD, QueueId::Console),
            (ONLINE_THREAD, QueueId::Online),
        ];
        for (name, queue_id) in threads {
            let make_executor = Arc::clone(&make_executor);
            let queues = Arc::clone(&queues);
            let outcomes = outcomes.clone();
            let thread = thread::Builder::new()
                .name(format!("{name} worker"))
                .spawn(move || work(&*make_executor, queues.queue(queue_id), &outcomes))?;
            match queue_id {
                QueueId::Online => runner.online_thread = Some(thread),
                QueueId::Load | QueueId::Console => runner.joined_threads.push(thread),
            }
        }
        Ok(runner)
    }

    /// A handle the app submits jobs through, while this runner stays with `main` for the shutdown.
    pub fn submitter(&self) -> JobSubmitter {
        JobSubmitter {
            queues: Arc::clone(&self.queues),
        }
    }

    /// Drops the jobs that have not started and waits for the running load and console jobs to end. The online
    /// thread is left to end on its own; dropping its handle detaches it.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.queues.close_all();
        self.online_thread = None;
        for thread in self.joined_threads.drain(..) {
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

fn work<E: JobExecutor>(
    make_executor: &(dyn Fn() -> E + Send + Sync),
    queue: &JobQueue,
    outcomes: &Sender<JobOutcome>,
) {
    let progress: ProgressSink = {
        let outcomes = outcomes.clone();
        Arc::new(move |outcome| send(&outcomes, outcome))
    };
    let mut executor = make_executor();
    while let Some(job) = queue.next() {
        let failed_job = FailedJob::of(&job);
        match panic::catch_unwind(AssertUnwindSafe(|| executor.execute(job, &progress))) {
            Ok(outcome) => send(outcomes, outcome),
            Err(payload) => {
                // The panic hook has logged the panic. The executor may hold an FTP session that is out of
                // step with the server, so the next job gets a fresh one.
                log::error!("A job on the {} thread panicked", thread_name());
                executor = make_executor();
                let panic = JobPanic::from_payload(payload.as_ref());
                send(
                    outcomes,
                    JobOutcome::Failed {
                        job: failed_job,
                        panic,
                    },
                );
            }
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
