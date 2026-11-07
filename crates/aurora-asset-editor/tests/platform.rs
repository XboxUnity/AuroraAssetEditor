use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use aurora_asset_editor::jobs::{
    ConsoleJob, ConsoleJobKind, ConsoleRequest, FailedJob, Job, JobExecutor, JobOutcome,
    JobPriority, JobRunner, Progress, ProgressSink, ThreadRunner,
};
use aurora_asset_editor::logging::{ERROR_LOG_FILE, ErrorLog, LogBridge, install_panic_hook};
use aurora_asset_editor::platform::{DropSignal, FileDropCollector};
use aurora_console::FtpSettings;
use slint::winit_030::winit::event::WindowEvent;

const GATE_JOB: u64 = 1;
const BACKGROUND_JOB: u64 = 2;
const SECOND_USER_JOB: u64 = 3;
const PANIC_JOB: u64 = 4;
const AFTER_PANIC_JOB: u64 = 5;
const WORKER_THREADS: usize = 3;

/// The panic hook and the logger are global: the tests that install them or panic take turns.
static GLOBAL_HOOKS: Mutex<()> = Mutex::new(());

#[test]
fn file_drop_collector_batches_one_drop() {
    let paths = ["a.asset", "b.png", "c.jpg"].map(PathBuf::from);
    let mut collector = FileDropCollector::default();

    assert_eq!(
        collector.on_event(&WindowEvent::HoveredFile(paths[0].clone())),
        DropSignal::Hover(true)
    );
    let signals: Vec<DropSignal> = paths
        .iter()
        .map(|path| collector.on_event(&WindowEvent::DroppedFile(path.clone())))
        .collect();

    assert_eq!(
        signals,
        [DropSignal::FirstFile, DropSignal::None, DropSignal::None]
    );
    assert_eq!(collector.take(), paths.to_vec());
    assert!(collector.take().is_empty());
    assert_eq!(
        collector.on_event(&WindowEvent::HoveredFileCancelled),
        DropSignal::Hover(false)
    );
}

/// Answers every job with a `Progress::Bulk` whose position is the job's refresh generation; the gate job
/// says it started and waits until the test opens the gate.
struct GatedExecutor {
    gate: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
    /// Told when the gate job starts.
    started: Option<mpsc::Sender<()>>,
}

impl JobExecutor for GatedExecutor {
    fn execute(&mut self, job: Job, _progress: &ProgressSink) -> JobOutcome {
        let generation = refresh_generation(&job);
        if generation == GATE_JOB {
            if let Some(started) = &self.started {
                started
                    .send(())
                    .expect("the test waits for the gate job to start");
            }
            let gate = self
                .gate
                .lock()
                .expect("the gate lock is never poisoned")
                .take();
            if let Some(gate) = gate {
                gate.recv().expect("the test opens the gate");
            }
        }
        JobOutcome::Progress(Progress::Bulk {
            position: generation as usize,
            total: 0,
            title: String::new(),
        })
    }
}

fn refresh_job(generation: u64) -> Job {
    Job::Console(ConsoleRequest {
        settings: FtpSettings::default(),
        job: ConsoleJob::Refresh { generation },
    })
}

fn refresh_generation(job: &Job) -> u64 {
    match job {
        Job::Console(ConsoleRequest {
            job: ConsoleJob::Refresh { generation },
            ..
        }) => *generation,
        other => panic!("the tests only submit refresh jobs, got {other:?}"),
    }
}

fn outcome_position(outcome: JobOutcome) -> u64 {
    match outcome {
        JobOutcome::Progress(Progress::Bulk { position, .. }) => position as u64,
        other => panic!("the test executor only answers with bulk progress, got {other:?}"),
    }
}

#[test]
fn thread_runner_runs_user_jobs_before_background() {
    let (open_gate, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let (sender, receiver) = async_channel::unbounded();
    let mut runner = ThreadRunner::new(
        move || GatedExecutor {
            gate: Arc::clone(&gate),
            started: None,
        },
        sender,
    )
    .expect("the test can start worker threads");

    runner.submit(refresh_job(GATE_JOB), JobPriority::User);
    runner.submit(refresh_job(BACKGROUND_JOB), JobPriority::Background);
    runner.submit(refresh_job(SECOND_USER_JOB), JobPriority::User);
    open_gate
        .send(())
        .expect("the console worker holds the gate");

    let order: Vec<u64> = (0..3)
        .map(|_| {
            outcome_position(
                receiver
                    .recv_blocking()
                    .expect("every job sends its outcome"),
            )
        })
        .collect();
    assert_eq!(order, [GATE_JOB, SECOND_USER_JOB, BACKGROUND_JOB]);
    runner.shutdown();
}

#[test]
fn outcomes_after_shutdown_are_quiet() {
    let _hooks = GLOBAL_HOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let config_dir = tempfile::TempDir::new().expect("tests need a temp folder for error.log");
    let log = ErrorLog::in_dir(config_dir.path());
    LogBridge::install(log.clone()).expect("no other test in this file installs a logger");
    install_panic_hook(log);
    let (open_gate, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let (report_start, started) = mpsc::channel();
    let (sender, receiver) = async_channel::unbounded();
    let mut runner = ThreadRunner::new(
        move || GatedExecutor {
            gate: Arc::clone(&gate),
            started: Some(report_start.clone()),
        },
        sender,
    )
    .expect("the test can start worker threads");

    runner.submit(refresh_job(GATE_JOB), JobPriority::User);
    started
        .recv()
        .expect("the console worker starts the gate job");
    drop(receiver);
    open_gate
        .send(())
        .expect("the console worker holds the gate");
    runner.shutdown();

    assert!(!config_dir.path().join(ERROR_LOG_FILE).exists());
}

/// Numbered in the order `make_executor` built it; it answers each job with a `Progress::Bulk` whose position
/// is its number, and panics on `PANIC_JOB` after the test opens the gate.
struct PanickingExecutor {
    number: usize,
    gate: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

impl JobExecutor for PanickingExecutor {
    fn execute(&mut self, job: Job, _progress: &ProgressSink) -> JobOutcome {
        if refresh_generation(&job) == PANIC_JOB {
            let gate = self
                .gate
                .lock()
                .expect("the gate lock is never poisoned")
                .take();
            if let Some(gate) = gate {
                gate.recv().expect("the test opens the gate");
            }
            panic!("executor {} broke", self.number);
        }
        JobOutcome::Progress(Progress::Bulk {
            position: self.number,
            total: 0,
            title: String::new(),
        })
    }
}

#[test]
fn panicking_job_fails_and_the_next_job_gets_a_fresh_executor() {
    let _hooks = GLOBAL_HOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let (open_gate, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let built = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = async_channel::unbounded();
    let counter = Arc::clone(&built);
    let mut runner = ThreadRunner::new(
        move || PanickingExecutor {
            number: counter.fetch_add(1, Ordering::SeqCst) + 1,
            gate: Arc::clone(&gate),
        },
        sender,
    )
    .expect("the test can start worker threads");

    runner.submit(refresh_job(PANIC_JOB), JobPriority::User);
    runner.submit(refresh_job(AFTER_PANIC_JOB), JobPriority::User);
    open_gate
        .send(())
        .expect("the console worker holds the gate");

    let failed = receiver
        .recv_blocking()
        .expect("the panicking job sends an outcome");
    let JobOutcome::Failed { job, panic } = failed else {
        panic!("a panicking job answers with Failed, got {failed:?}");
    };
    assert_eq!(job, FailedJob::ConsoleAction(ConsoleJobKind::Refresh));
    let broken = panic
        .to_string()
        .strip_prefix("executor ")
        .and_then(|rest| rest.strip_suffix(" broke"))
        .and_then(|number| number.parse::<usize>().ok())
        .expect("the panic message names the executor");
    let next = outcome_position(
        receiver
            .recv_blocking()
            .expect("the job after the panic runs"),
    ) as usize;
    assert_ne!(next, broken);
    runner.shutdown();
    // One executor per thread, plus the one built after the panic.
    assert_eq!(built.load(Ordering::SeqCst), WORKER_THREADS + 1);
}
