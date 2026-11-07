use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use aurora_asset_editor::about::BuildInfo;
use aurora_asset_editor::app::{App, AppEnvironment, StartupProblem, StartupSettings};
use aurora_asset_editor::jobs::{Job, JobOutcome, JobWorkers, ProgressSink};
use aurora_asset_editor::logging::{ERROR_LOG_FILE, ErrorLog};
use aurora_asset_editor::settings::{AppSettings, ThemeChoice};
use aurora_console::{FtpSettings, Timeouts};
use aurora_online::{Endpoints, OnlineClient};
use tempfile::TempDir;
use test_servers::FakeConsole;

use crate::{
    FakeClipboard, FakeClipboardHandle, FakeDialogs, FakeDialogsHandle, FakeFolderOpener,
    FakeFolderOpenerHandle, FixedClock, QueuedRunner,
};

pub const TEST_SUBNET_PREFIX: &str = "192.168.1.";
const TEST_ONLINE_TIMEOUT: Duration = Duration::from_secs(5);
/// Port 9 (discard) has no listener, so a request that is not routed to a test server is refused (on Windows
/// a refused connect takes about 2 s).
const REFUSING_URL: &str = "http://127.0.0.1:9/";
const FAKE_CONSOLE_HOST: &str = "127.0.0.1";

/// The build every harness app reports.
pub fn test_build_info() -> BuildInfo {
    BuildInfo {
        version: "2.0.7".to_owned(),
        short_hash: "a1b2c3d".to_owned(),
        full_hash: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_owned(),
        dirty: false,
        build_date: "2026-11-07".to_owned(),
        target: "x86_64-pc-windows-msvc".to_owned(),
        run_url: None,
    }
}

/// When a `run_jobs_with` hook is called: after a progress outcome (the job waits meanwhile) or after the
/// job's final outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStage {
    Progress,
    Final,
}

struct Setup {
    environment: AppEnvironment,
    online: OnlineClient,
}

struct Running {
    app: App,
    workers: JobWorkers,
}

/// An `App` with fake platform services in a temporary config folder. The app is built on the first call
/// to `app()`; the `with_*` methods change what it is built with and must come first.
pub struct AppHarness {
    config_dir: TempDir,
    dialogs: FakeDialogsHandle,
    clipboard: FakeClipboardHandle,
    folder_opener: FakeFolderOpenerHandle,
    jobs: QueuedRunner,
    setup: Option<Setup>,
    running: Option<Running>,
}

impl AppHarness {
    pub fn new() -> AppHarness {
        let config_dir = TempDir::new().expect("tests need a temp folder for the config files");
        let (dialogs, dialogs_handle) = FakeDialogs::new();
        let (clipboard, clipboard_handle) = FakeClipboard::new();
        let (folder_opener, folder_opener_handle) = FakeFolderOpener::new();
        let jobs = QueuedRunner::new();
        let environment = AppEnvironment {
            config_dir: config_dir.path().to_path_buf(),
            settings: AppSettings {
                theme: ThemeChoice::Dark,
                ..AppSettings::default()
            },
            keep_settings_file: false,
            ftp_settings: None,
            subnet_prefix: TEST_SUBNET_PREFIX.to_owned(),
            build: test_build_info(),
            error_log: ErrorLog::in_dir(config_dir.path()),
            dialogs: Box::new(dialogs),
            clipboard: Box::new(clipboard),
            folder_opener: Box::new(folder_opener),
            clock: Box::new(FixedClock::harness_time()),
            jobs: Box::new(jobs.clone()),
            startup_problems: Vec::new(),
        };
        let offline = Endpoints {
            xboxunity_covers: REFUSING_URL.to_owned(),
            archive_covers: REFUSING_URL.to_owned(),
        };
        AppHarness {
            config_dir,
            dialogs: dialogs_handle,
            clipboard: clipboard_handle,
            folder_opener: folder_opener_handle,
            jobs,
            setup: Some(Setup {
                environment,
                online: OnlineClient::without_proxy(offline, TEST_ONLINE_TIMEOUT),
            }),
            running: None,
        }
    }

    pub fn app(&mut self) -> &mut App {
        &mut self.running().app
    }

    pub fn config_dir(&self) -> &Path {
        self.config_dir.path()
    }

    /// FTP settings for `console` on 127.0.0.1, with the fake console's default credentials.
    pub fn with_console(&mut self, console: &FakeConsole) {
        self.setup("with_console").environment.ftp_settings = Some(FtpSettings {
            host: FAKE_CONSOLE_HOST.to_owned(),
            port: console.port().to_string(),
            ..FtpSettings::default()
        });
    }

    pub fn with_online(&mut self, endpoints: Endpoints) {
        self.setup("with_online").online =
            OnlineClient::without_proxy(endpoints, TEST_ONLINE_TIMEOUT);
    }

    /// Starts the app with `startup` in place of the harness's settings, as `main` does.
    pub fn with_startup_settings(&mut self, startup: StartupSettings) {
        let environment = &mut self.setup("with_startup_settings").environment;
        environment.settings = startup.settings;
        environment.keep_settings_file = startup.keep_file;
        environment.startup_problems.extend(startup.problem);
    }

    pub fn with_startup_problem(&mut self, problem: StartupProblem) {
        self.setup("with_startup_problem")
            .environment
            .startup_problems
            .push(problem);
    }

    pub fn dialogs(&self) -> FakeDialogsHandle {
        self.dialogs.clone()
    }

    pub fn clipboard(&self) -> FakeClipboardHandle {
        self.clipboard.clone()
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard.text()
    }

    pub fn opened_folders(&self) -> Vec<PathBuf> {
        self.folder_opener.opened()
    }

    /// The whole `error.log`; empty when nothing was logged.
    pub fn error_log_text(&self) -> String {
        match std::fs::read_to_string(self.config_dir().join(ERROR_LOG_FILE)) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => panic!("error.log is unreadable: {error}"),
        }
    }

    pub fn queued_jobs(&self) -> usize {
        self.jobs.queued()
    }

    /// Runs queued jobs, `User` before `Background`, until none is left, jobs queued meanwhile included.
    pub fn run_jobs(&mut self) {
        self.run_jobs_with(|_, _| {});
    }

    /// Like `run_jobs`, calling `hook` right after each outcome, progress included, has been applied.
    pub fn run_jobs_with(&mut self, mut hook: impl FnMut(&mut App, OutcomeStage)) {
        self.running();
        while let Some(job) = self.jobs.next_job() {
            let running = self
                .running
                .as_mut()
                .expect("the app was built before the first job");
            run_job(running, job, &mut hook);
        }
    }

    fn setup(&mut self, method: &str) -> &mut Setup {
        self.setup
            .as_mut()
            .unwrap_or_else(|| panic!("{method} must be called before app()"))
    }

    fn running(&mut self) -> &mut Running {
        let setup = &mut self.setup;
        self.running.get_or_insert_with(|| {
            let setup = setup
                .take()
                .expect("the setup is kept until the app is built");
            Running {
                app: App::new(setup.environment),
                workers: JobWorkers::new(Timeouts::default(), setup.online),
            }
        })
    }
}

impl Default for AppHarness {
    fn default() -> Self {
        Self::new()
    }
}

enum WorkerMessage {
    Progress(JobOutcome),
    Finished(thread::Result<JobOutcome>),
}

/// Runs `job` on a scoped thread and applies its outcomes on this one. The job waits after each progress
/// outcome until it has been applied and the hook has run, so a hook acts at that point of the job, as a
/// user can between two progress updates.
fn run_job(running: &mut Running, job: Job, hook: &mut impl FnMut(&mut App, OutcomeStage)) {
    let Running { app, workers } = running;
    thread::scope(|scope| {
        let (sender, messages) = mpsc::channel();
        let (resume, resumed) = mpsc::channel::<()>();
        let progress_sender = sender.clone();
        let resumed = Mutex::new(resumed);
        let progress: ProgressSink = Arc::new(move |outcome| {
            // Both channels fail only when the test thread has stopped on a failed assertion, which
            // reports itself; the job then runs to its end unobserved.
            if progress_sender
                .send(WorkerMessage::Progress(outcome))
                .is_ok()
            {
                let _resumed_or_test_gone = resumed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .recv();
            }
        });
        scope.spawn(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(|| workers.execute(job, &progress)));
            drop(progress);
            let _test_gone = sender.send(WorkerMessage::Finished(result));
        });
        loop {
            let message = messages
                .recv()
                .expect("the worker sends Finished before it ends");
            match message {
                WorkerMessage::Progress(outcome) => {
                    app.on_job_outcome(outcome);
                    hook(app, OutcomeStage::Progress);
                    resume
                        .send(())
                        .expect("the job waits for its progress to be applied");
                }
                WorkerMessage::Finished(Ok(outcome)) => {
                    app.on_job_outcome(outcome);
                    hook(app, OutcomeStage::Final);
                    break;
                }
                WorkerMessage::Finished(Err(panic_payload)) => panic::resume_unwind(panic_payload),
            }
        }
    });
}
