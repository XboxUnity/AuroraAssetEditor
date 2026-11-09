#![forbid(unsafe_code)]
// A release build is a window app: no console window opens with it on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use async_channel::Receiver;
use aurora_asset_editor::about::BuildInfo;
use aurora_asset_editor::app::{
    App, AppEnvironment, LOG_WRITE_FAILED, StartupProblem, StartupSettings,
};
use aurora_asset_editor::jobs::{FailedJob, JobOutcome, JobWorkers, ThreadRunner};
use aurora_asset_editor::logging::{ErrorLog, LogBridge, error_chain, install_panic_hook};
use aurora_asset_editor::platform::{
    DropSignal, FileDropCollector, NativeFileDialogs, SystemClipboard, SystemClock,
    SystemFolderOpener, SystemTheme, window_placement,
};
use aurora_asset_editor::settings::WindowGeometry;
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
use slint::{ComponentHandle, Timer};

const VERSION_FLAG: &str = "--version";
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

/// Everything the window needs once it exists on screen.
struct WindowSetup {
    icon: Option<Icon>,
    geometry: Option<WindowGeometry>,
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if arguments == [VERSION_FLAG] {
        return print_version();
    }
    let mut problems = Vec::new();
    let config_dir = config_dir(&mut problems);
    let error_log = ErrorLog::in_dir(&config_dir);
    install_panic_hook(error_log.clone());
    if let Err(error) = LogBridge::install(error_log.clone()) {
        problems.push(startup_problem(LOGGER_FAILED, error));
    }
    let paths = arguments.into_iter().map(PathBuf::from).collect();
    match run(config_dir, error_log.clone(), paths, problems) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report_outside_event_loop(&error_log, STARTUP_FAILED, error.as_ref());
            ExitCode::FAILURE
        }
    }
}

fn run(
    config_dir: PathBuf,
    error_log: ErrorLog,
    paths: Vec<PathBuf>,
    mut problems: Vec<StartupProblem>,
) -> Result<(), Box<dyn Error>> {
    let startup = StartupSettings::load(&config_dir);
    problems.extend(startup.problem);
    let ftp_settings = load_ftp_settings(&config_dir, &mut problems);
    let subnet_prefix = subnet_prefix(&mut problems);
    let icon = app_icon(&mut problems);
    let geometry = startup.settings.window;

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
        dialogs: Box::new(NativeFileDialogs::new(window.as_weak())),
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
