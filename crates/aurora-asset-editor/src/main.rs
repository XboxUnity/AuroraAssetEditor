#![forbid(unsafe_code)]
// A release build is a window app: no console window opens with it on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use aurora_asset::{AssetFile, AssetKind};
use aurora_asset_editor::about::BuildInfo;
use aurora_asset_editor::app::{
    App, AppEnvironment, LOG_WRITE_FAILED, StartupProblem, StartupSettings,
};
use aurora_asset_editor::jobs::{FailedJob, JobOutcome, JobWorkers, ThreadRunner};
use aurora_asset_editor::launch_mode::{LaunchMode, LaunchModeError};
use aurora_asset_editor::logging::{
    ERROR_LOG_FILE, ErrorLog, LogBridge, error_chain, install_panic_hook,
};
#[cfg(target_os = "macos")]
use aurora_asset_editor::platform::app_menu::{self, AppMenu};
use aurora_asset_editor::platform::{
    DropSignal, FileDialogs, FileDropCollector, NativeFileDialogs, SystemClipboard, SystemClock,
    SystemFolderOpener, SystemTheme, window_placement,
};
use aurora_asset_editor::settings::WindowGeometry;
use aurora_asset_editor::smoke_test::{
    SMOKE_POLL_INTERVAL, SmokeStep, SmokeTestDialogs, SmokeTestDriver,
};
use aurora_asset_editor::view::{MainWindowView, run_when_app_free};
use aurora_console::{
    FTP_SETTINGS_FILE, FtpSettings, Timeouts, app_config_dir, local_subnet_prefix,
};
use aurora_online::{DEFAULT_TIMEOUT, Endpoints, OnlineClient};
use aurora_ui::MainWindow;
use slint::winit_030::winit::event::WindowEvent;
#[cfg(windows)]
use slint::winit_030::winit::platform::windows::WindowExtWindows;
use slint::winit_030::winit::window::{Icon, Theme};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, Timer, TimerMode};
use tempfile::TempDir;

const APP_ICON: &[u8] = include_bytes!("../../aurora-ui/ui/app_icon.ico");
const APP_ICON_NAME: &str = "app_icon.ico";

const STARTUP_FAILED: &str = "The app could not start";
const NO_CONFIG_DIR: &str =
    "The system has no folder for app settings, so the current folder is used";
const LOGGER_FAILED: &str = "Could not send log messages to error.log";
const FTP_SETTINGS_FAILED: &str = "Could not load the console connection settings";
const SUBNET_PREFIX_FAILED: &str = "Could not find the local network to suggest a console address";
const APP_ICON_FAILED: &str = "Could not load the window icon";
const WINDOW_SETUP_FAILED: &str = "Could not finish setting up the window";
const HIDE_FAILED: &str = "Could not close the window";
const MESSAGE_BOX_FAILED: &str = "Could not show the error in a message box";
const NO_CONSOLE_FOR_VERSION: &str = "There is no console to print the version to";
const ARGUMENTS_REFUSED: &str = "The command line was not understood";
const SMOKE_TEST_FAILED: &str = "Smoke test failed";
const SMOKE_CONFIG_FAILED: &str = "Could not create the temporary settings folder";
const SMOKE_CONFIG_NOT_DELETED: &str =
    "Could not delete the smoke test's temporary settings folder";
const SMOKE_WINDOW_CLOSED: &str = "The window closed before the smoke test finished";
const SMOKE_QUIT_FAILED: &str = "Could not end the smoke test's event loop";
#[cfg(target_os = "macos")]
const APP_MENU_FAILED: &str = "Could not set up the app menu";

/// Everything the window needs once it exists on screen.
struct WindowSetup {
    icon: Option<Icon>,
    geometry: Option<WindowGeometry>,
    paths: Vec<PathBuf>,
}

/// Where the app keeps its settings and log, the files to load, and what went wrong before the window exists.
struct Launch {
    config_dir: PathBuf,
    error_log: ErrorLog,
    paths: Vec<PathBuf>,
    problems: Vec<StartupProblem>,
}

fn main() -> ExitCode {
    match LaunchMode::from_arguments(env::args_os().skip(1).collect()) {
        Ok(LaunchMode::Version) => print_version(),
        Ok(LaunchMode::Window { paths }) => open_window(paths),
        Ok(LaunchMode::SmokeTest { asset, output }) => run_smoke_test(&asset, output),
        Err(error) => refuse_arguments(&error),
    }
}

fn open_window(paths: Vec<PathBuf>) -> ExitCode {
    let mut problems = Vec::new();
    let config_dir = config_dir(&mut problems);
    let error_log = start_logging(&config_dir, &mut problems);
    let launch = Launch {
        config_dir,
        error_log: error_log.clone(),
        paths,
        problems,
    };
    let native_dialogs = |window: &MainWindow| -> Box<dyn FileDialogs> {
        Box::new(NativeFileDialogs::new(window.as_weak()))
    };
    match run(launch, native_dialogs, |_, _| {}) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report_outside_event_loop(&error_log, STARTUP_FAILED, error.as_ref());
            ExitCode::FAILURE
        }
    }
}

fn start_logging(config_dir: &Path, problems: &mut Vec<StartupProblem>) -> ErrorLog {
    let error_log = ErrorLog::in_dir(config_dir);
    install_panic_hook(error_log.clone());
    if let Err(error) = LogBridge::install(error_log.clone()) {
        problems.push(startup_problem(LOGGER_FAILED, error));
    }
    error_log
}

/// `make_dialogs` gets the window the dialogs belong to; `once_running` gets the app and its view just before
/// the event loop starts.
fn run(
    launch: Launch,
    make_dialogs: impl FnOnce(&MainWindow) -> Box<dyn FileDialogs>,
    once_running: impl FnOnce(&Rc<RefCell<App>>, &Rc<MainWindowView>),
) -> Result<(), Box<dyn Error>> {
    let Launch {
        config_dir,
        error_log,
        paths,
        mut problems,
    } = launch;
    let startup = StartupSettings::load(&config_dir);
    problems.extend(startup.problem);
    let ftp_settings = load_ftp_settings(&config_dir, &mut problems);
    let subnet_prefix = subnet_prefix(&mut problems);
    let icon = app_icon(&mut problems);
    let geometry = startup.settings.window;

    #[cfg(target_os = "macos")]
    app_menu::use_backend_without_default_menu()?;
    let window = MainWindow::new()?;
    let (outcomes, outcome_receiver) = async_channel::unbounded();
    let outcomes_after_close = outcome_receiver.clone();
    let timeouts = Timeouts::default();
    let online = OnlineClient::new(Endpoints::live(), DEFAULT_TIMEOUT);
    let runner = ThreadRunner::new(move || JobWorkers::new(timeouts, online.clone()), outcomes)?;
    let exit_log = error_log.clone();
    let environment = AppEnvironment {
        config_dir,
        settings: startup.settings,
        keep_settings_file: startup.keep_file,
        ftp_settings,
        subnet_prefix,
        build: BuildInfo::current(),
        error_log,
        dialogs: make_dialogs(&window),
        clipboard: Box::new(SystemClipboard::default()),
        folder_opener: Box::new(SystemFolderOpener),
        clock: Box::new(SystemClock),
        jobs: Box::new(runner.submitter()),
        startup_problems: problems,
    };
    let app = Rc::new(RefCell::new(App::new(environment)));
    let view = Rc::new(MainWindowView::new(window, Box::new(runner.submitter())));
    view.connect(Rc::clone(&app));
    receive_outcomes(outcome_receiver, &app, &view)?;
    watch_window_events(&app, &view);
    view.connect_close(&app, save_geometry);

    window_placement::restore_size(view.window().window(), geometry.as_ref());
    view.window().show()?;
    let setup = WindowSetup {
        icon,
        geometry,
        paths,
    };
    finish_window_setup(&app, &view, setup)?;
    #[cfg(target_os = "macos")]
    let _app_menu = install_app_menu(&app, &view);
    once_running(&app, &view);
    slint::run_event_loop()?;
    // A bulk run would otherwise keep the closed app running until its last game.
    app.borrow_mut().stop_bulk();
    if let Err(error) = view.window().hide() {
        report_outside_event_loop(&exit_log, HIDE_FAILED, &error);
    }
    runner.shutdown();
    app.borrow_mut()
        .apply_outcomes_after_close(&outcomes_after_close);
    Ok(())
}

/// The command line was not understood: shown like any other startup failure.
fn refuse_arguments(error: &LaunchModeError) -> ExitCode {
    let text = error.to_string();
    if let Err(box_error) = aurora_file_dialogs::show_startup_error(ARGUMENTS_REFUSED, &text) {
        eprintln!("{ARGUMENTS_REFUSED}: {text} ({MESSAGE_BOX_FAILED}: {box_error})");
    }
    ExitCode::from(2)
}

/// `--smoke-test`: the real window and event loop with a temporary settings folder, driven by
/// `SmokeTestDriver`. Nobody is there to answer a message box, so every failure goes to stderr.
fn run_smoke_test(asset: &Path, output: PathBuf) -> ExitCode {
    let config = match TempDir::new() {
        Ok(folder) => folder,
        Err(error) => {
            eprintln!("{SMOKE_TEST_FAILED}: {SMOKE_CONFIG_FAILED}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut problems = Vec::new();
    let error_log = start_logging(config.path(), &mut problems);
    let launch = Launch {
        config_dir: config.path().to_path_buf(),
        error_log: error_log.clone(),
        paths: Vec::new(),
        problems,
    };
    let exit_code = match smoke_test_result(launch, asset, output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            report_smoke_failure(&error_log, &reason);
            ExitCode::FAILURE
        }
    };
    if let Err(error) = config.close() {
        eprintln!("{SMOKE_CONFIG_NOT_DELETED}: {error}");
    }
    exit_code
}

/// The reason the smoke test failed, when it did.
fn smoke_test_result(launch: Launch, asset: &Path, output: PathBuf) -> Result<(), String> {
    let kind = asset_kind(asset).map_err(|error| {
        let reason = error_chain(error.as_ref()).join(": ");
        format!("Could not use {} as the asset: {reason}", asset.display())
    })?;
    let driver = SmokeTestDriver::new(asset.to_path_buf(), output.clone(), kind);
    let timer = Timer::default();
    let finished = Rc::new(RefCell::new(None));
    let smoke_dialogs =
        |_: &MainWindow| -> Box<dyn FileDialogs> { Box::new(SmokeTestDialogs::new(output)) };
    run(launch, smoke_dialogs, |app, view| {
        start_smoke_driver(&timer, driver, app, view, &finished);
    })
    .map_err(|error| error_chain(error.as_ref()).join(": "))?;
    match finished.take() {
        Some(SmokeStep::Passed) => Ok(()),
        Some(SmokeStep::Failed(reason)) => Err(reason),
        Some(SmokeStep::Waiting) | None => Err(SMOKE_WINDOW_CLOSED.to_owned()),
    }
}

/// The kind of the asset's first image, which decides the page it loads onto and is saved from.
fn asset_kind(asset: &Path) -> Result<AssetKind, Box<dyn Error>> {
    let file = AssetFile::parse(&fs::read(asset)?)?;
    let kind = file
        .primary_kind()
        .ok_or_else(|| format!("{} holds no image", asset.display()))?;
    Ok(kind)
}

/// Steps the driver every `SMOKE_POLL_INTERVAL` and shows each step in the window. Once the driver passes or
/// fails, its result goes to `finished` and the event loop ends.
fn start_smoke_driver(
    timer: &Timer,
    driver: SmokeTestDriver,
    app: &Rc<RefCell<App>>,
    view: &Rc<MainWindowView>,
    finished: &Rc<RefCell<Option<SmokeStep>>>,
) {
    let app = Rc::downgrade(app);
    let view = Rc::downgrade(view);
    let driver = Rc::new(RefCell::new(driver));
    let finished = Rc::clone(finished);
    let started = Instant::now();
    timer.start(TimerMode::Repeated, SMOKE_POLL_INTERVAL, move || {
        let view = view.clone();
        let driver = Rc::clone(&driver);
        let finished = Rc::clone(&finished);
        run_when_app_free(app.clone(), move |app| {
            let mut finished = finished.borrow_mut();
            if finished.is_none() {
                let step = driver.borrow_mut().step(app, started.elapsed());
                if let Some(view) = view.upgrade() {
                    view.render(app);
                }
                if step == SmokeStep::Waiting {
                    return;
                }
                *finished = Some(step);
            }
            // When this fails, the next tick asks again.
            if let Err(error) = slint::quit_event_loop() {
                eprintln!("{SMOKE_QUIT_FAILED}: {error}");
            }
        });
    });
}

/// The temporary folder goes when the run ends, so the reason and everything else the run logged are copied
/// to stderr.
fn report_smoke_failure(error_log: &ErrorLog, reason: &str) {
    eprintln!("{SMOKE_TEST_FAILED}: {reason}");
    if let Err(error) = error_log.record_message(SMOKE_TEST_FAILED, reason) {
        eprintln!("{LOG_WRITE_FAILED}: {error}");
    }
    match fs::read_to_string(error_log.path()) {
        Ok(text) => eprintln!("The smoke test's {ERROR_LOG_FILE}:\n{text}"),
        Err(error) => eprintln!("Could not read the smoke test's {ERROR_LOG_FILE}: {error}"),
    }
}

/// A release build is a window app, which Windows starts without a console; when it cannot join the one it
/// was started from, the version is shown in a message box instead.
fn print_version() -> ExitCode {
    let version = BuildInfo::current().window_title();
    if let Err(error) = aurora_file_dialogs::attach_parent_console() {
        let text = format!("{NO_CONSOLE_FOR_VERSION}: {error}");
        return match aurora_file_dialogs::show_startup_error(&version, &text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(box_error) => {
                eprintln!("{version}\n{text} ({MESSAGE_BOX_FAILED}: {box_error})");
                ExitCode::FAILURE
            }
        };
    }
    println!("{version}");
    ExitCode::SUCCESS
}

/// For a failure while no window can show it: it is logged and shown in a message box. When even that fails,
/// stderr is the last place left.
fn report_outside_event_loop(error_log: &ErrorLog, context: &str, error: &(dyn Error + 'static)) {
    let mut text = error_chain(error).join(": ");
    if let Err(log_error) = error_log.record(context, error) {
        text.push_str(&format!("\n\n{LOG_WRITE_FAILED}: {log_error}"));
    }
    if let Err(box_error) = aurora_file_dialogs::show_startup_error(context, &text) {
        eprintln!("{context}: {text} ({MESSAGE_BOX_FAILED}: {box_error})");
        if let Err(log_error) = error_log.record(MESSAGE_BOX_FAILED, &box_error) {
            eprintln!("{LOG_WRITE_FAILED}: {log_error}");
        }
    }
}

/// Like the old app, which kept `ftp.json` next to itself, the current folder is used when the system has none.
fn config_dir(problems: &mut Vec<StartupProblem>) -> PathBuf {
    match app_config_dir() {
        Some(dir) => dir,
        None => {
            problems.push(startup_problem(
                NO_CONFIG_DIR,
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "no config folder is known for this user",
                ),
            ));
            PathBuf::new()
        }
    }
}

fn load_ftp_settings(config_dir: &Path, problems: &mut Vec<StartupProblem>) -> Option<FtpSettings> {
    match FtpSettings::load(&config_dir.join(FTP_SETTINGS_FILE)) {
        Ok(settings) => settings,
        Err(error) => {
            problems.push(startup_problem(FTP_SETTINGS_FAILED, error));
            None
        }
    }
}

/// Pre-fills the console address; without one the host starts empty.
fn subnet_prefix(problems: &mut Vec<StartupProblem>) -> String {
    match local_subnet_prefix() {
        Ok(prefix) => prefix,
        Err(error) => {
            problems.push(startup_problem(SUBNET_PREFIX_FAILED, error));
            String::new()
        }
    }
}

/// ICO is an import format, and the decoder picks the icon's largest image.
fn app_icon(problems: &mut Vec<StartupProblem>) -> Option<Icon> {
    let decoded = aurora_image::decode_image(APP_ICON, Some(APP_ICON_NAME))
        .map_err(|error| startup_problem(APP_ICON_FAILED, error))
        .and_then(|image| {
            let (width, height) = image.dimensions();
            Icon::from_rgba(image.into_raw(), width, height)
                .map_err(|error| startup_problem(APP_ICON_FAILED, error))
        });
    match decoded {
        Ok(icon) => Some(icon),
        Err(problem) => {
            problems.push(problem);
            None
        }
    }
}

fn startup_problem(
    context: &str,
    error: impl Into<Box<dyn Error + Send + Sync>>,
) -> StartupProblem {
    StartupProblem {
        context: context.to_owned(),
        error: error.into(),
    }
}

fn receive_outcomes(
    receiver: Receiver<JobOutcome>,
    app: &Rc<RefCell<App>>,
    view: &Rc<MainWindowView>,
) -> Result<(), slint::EventLoopError> {
    let app = Rc::clone(app);
    let view = Rc::clone(view);
    slint::spawn_local(async move {
        while let Ok(outcome) = receiver.recv().await {
            match outcome {
                JobOutcome::Preview(resized) => view.show_resized(resized),
                outcome => {
                    // The app reports the failure; the view asks for the picture again.
                    if let JobOutcome::Failed {
                        job:
                            FailedJob::Preview {
                                picture,
                                area,
                                size,
                            },
                        ..
                    } = &outcome
                    {
                        view.preview_failed(*picture, *area, *size);
                    }
                    app.borrow_mut().on_job_outcome(outcome);
                }
            }
            view.render(&app.borrow());
            let close_approved = app.borrow().close_approved();
            if close_approved {
                view.request_close(&app);
            }
        }
    })?;
    Ok(())
}

/// Dropped files and theme changes come from winit, which reports each dropped file as its own event.
fn watch_window_events(app: &Rc<RefCell<App>>, view: &Rc<MainWindowView>) {
    let app = Rc::clone(app);
    let events_view = Rc::clone(view);
    let collector = Rc::new(RefCell::new(FileDropCollector::default()));
    view.window()
        .window()
        .on_winit_window_event(move |_window, event| {
            if let WindowEvent::ThemeChanged(theme) = event {
                let mut changed = events_view.system_theme();
                changed.update(*theme == Theme::Dark);
                events_view.set_system_theme(changed);
                events_view.render(&app.borrow());
                return EventResult::Propagate;
            }
            let signal = collector.borrow_mut().on_event(event);
            match signal {
                DropSignal::None => {}
                DropSignal::Hover(hover) => {
                    app.borrow_mut().set_drop_hover(hover);
                    events_view.render(&app.borrow());
                }
                DropSignal::FirstFile => {
                    let app = Rc::downgrade(&app);
                    let view = Rc::clone(&events_view);
                    let collector = Rc::clone(&collector);
                    // Runs after the event loop has delivered the drop's other files, so one drop is one batch.
                    Timer::single_shot(Duration::ZERO, move || {
                        run_when_app_free(app, move |app| {
                            let paths = collector.borrow_mut().take();
                            app.files_dropped(paths);
                            view.render(app);
                        });
                    });
                }
            }
            EventResult::Propagate
        });
}

/// The macOS app menu; without it the app still runs, and the window's close button still asks.
#[cfg(target_os = "macos")]
fn install_app_menu(app: &Rc<RefCell<App>>, view: &Rc<MainWindowView>) -> Option<AppMenu> {
    let about_app = Rc::downgrade(app);
    let about_view = Rc::downgrade(view);
    let about = move || {
        let view = about_view.clone();
        run_when_app_free(about_app.clone(), move |app| {
            app.show_about();
            if let Some(view) = view.upgrade() {
                view.render(app);
            }
        });
    };
    let quit_app = Rc::downgrade(app);
    let quit_view = Rc::downgrade(view);
    let quit = move || {
        if let (Some(view), Some(app)) = (quit_view.upgrade(), quit_app.upgrade()) {
            view.request_close(&app);
        }
    };
    match AppMenu::install(about, quit) {
        Ok(menu) => Some(menu),
        Err(error) => {
            app.borrow_mut()
                .report_startup_problem(startup_problem(APP_MENU_FAILED, error));
            None
        }
    }
}

fn save_geometry(app: &mut App, window: &slint::Window) {
    let saved = app.settings().window;
    let geometry = window_placement::capture(window, saved.as_ref());
    app.save_window_geometry(geometry);
}

/// Waits for the system window, which exists only once the event loop runs: the icon and position need it,
/// and the OS theme is read before the first render, which overwrites the palette it is read from.
fn finish_window_setup(
    app: &Rc<RefCell<App>>,
    view: &Rc<MainWindowView>,
    setup: WindowSetup,
) -> Result<(), slint::EventLoopError> {
    let app = Rc::clone(app);
    let view = Rc::clone(view);
    slint::spawn_local(async move {
        match view.window().window().winit_window().await {
            Ok(winit_window) => {
                #[cfg(windows)]
                winit_window.set_taskbar_icon(setup.icon.clone());
                winit_window.set_window_icon(setup.icon);
            }
            Err(error) => app
                .borrow_mut()
                .report_startup_problem(startup_problem(WINDOW_SETUP_FAILED, error)),
        }
        window_placement::restore_position(view.window().window(), setup.geometry.as_ref());
        let initial_theme = SystemTheme::read_initial(view.window());
        view.set_system_theme(initial_theme);
        view.render(&app.borrow());
        app.borrow_mut().open_paths(setup.paths);
        view.render(&app.borrow());
    })?;
    Ok(())
}
