use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use aurora_asset_editor::jobs::{
    ConsoleJob, ConsoleRequest, Job, JobExecutor, JobOutcome, JobPriority, JobRunner, Progress,
    ProgressSink, ThreadRunner,
};
use aurora_asset_editor::logging::{ERROR_LOG_FILE, ErrorLog, LogBridge, install_panic_hook};
use aurora_asset_editor::platform::{DropSignal, FileDropCollector};
use aurora_console::FtpSettings;
use slint::winit_030::winit::event::WindowEvent;

const GATE_JOB: u64 = 1;
const BACKGROUND_JOB: u64 = 2;
const SECOND_USER_JOB: u64 = 3;

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
        || GatedExecutor {
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
    let config_dir = tempfile::TempDir::new().expect("tests need a temp folder for error.log");
    let log = ErrorLog::in_dir(config_dir.path());
    LogBridge::install(log.clone()).expect("no other test in this file installs a logger");
    install_panic_hook(log);
    let (open_gate, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let (report_start, started) = mpsc::channel();
    let (sender, receiver) = async_channel::unbounded();
    let mut runner = ThreadRunner::new(
        || GatedExecutor {
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
