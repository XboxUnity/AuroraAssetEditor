use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use aurora_asset_editor::about::{BuildInfo, notices_text};
use aurora_asset_editor::app::{ABOUT_TITLE, App, COPY_BUILD_INFO_FAILED, OPEN_LOG_FOLDER_FAILED};
use aurora_asset_editor::dialogs::Modal;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::MainWindowView;
use aurora_ui::{DialogKind, DialogModel, MainWindow, ShellModel};
use editor_test_support::{AppHarness, HeadlessUi};
use i_slint_backend_testing::ElementHandle;
use image::RgbaImage;
use slint::ComponentHandle;

/// Set to a folder to keep the rendered dialog as PNG files for a look.
const SNAPSHOT_DIR_VARIABLE: &str = "AURORA_UI_SNAPSHOT_DIR";
const ABOUT_SLINT_ID: &str = "AboutDialog::about-slint";
/// The widget draws its logo and text in at least this many colours besides the card's.
const MIN_FOREGROUND_COLOURS: usize = 2;
const CARD_PROBE_GAP: f32 = 4.0;
const DATE_EXAMPLE: &str = "2026-11-07T10:00:00Z";
const THEMES: [(ThemeChoice, &str); 2] =
    [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")];

fn build_info(run_url: Option<&str>) -> BuildInfo {
    BuildInfo {
        version: "2.0.7".to_owned(),
        short_hash: "a1b2c3d".to_owned(),
        full_hash: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_owned(),
        dirty: false,
        build_date: DATE_EXAMPLE.to_owned(),
        target: "x86_64-pc-windows-msvc".to_owned(),
        run_url: run_url.map(str::to_owned),
    }
}

#[test]
fn copy_build_info_lists_every_field() {
    let mut harness = AppHarness::new();
    harness.app().copy_build_info();
    let copied = harness.clipboard_text().expect("the build info was copied");
    let lines: Vec<&str> = copied.lines().collect();
    assert_eq!(lines.len(), 4, "{copied}");
    assert_eq!(lines[0], "Aurora Asset Editor 2.0.7 (a1b2c3d)");
    assert!(lines[1].starts_with("Commit: a1b2c3d4"));
    assert!(lines[2].starts_with("Built: "));
    assert!(lines[3].starts_with("Target: "));

    let with_run = build_info(Some("https://example.test/runs/7")).copy_text();
    assert_eq!(with_run.lines().count(), 5);
    assert_eq!(
        with_run.lines().last(),
        Some("Run: https://example.test/runs/7")
    );
}

#[test]
fn current_build_info_is_filled() {
    let build = BuildInfo::current();
    assert!(build.version.starts_with("2."), "{}", build.version);
    let hash_ok = build.short_hash == "unknown"
        || (build.short_hash.len() >= 7 && build.short_hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(hash_ok, "{}", build.short_hash);
    let date = &build.build_date;
    assert_eq!(date.len(), DATE_EXAMPLE.len(), "{date}");
    for (index, (actual, example)) in date.chars().zip(DATE_EXAMPLE.chars()).enumerate() {
        if example.is_ascii_digit() {
            assert!(actual.is_ascii_digit(), "{date} at {index}");
        } else {
            assert_eq!(actual, example, "{date} at {index}");
        }
    }
    assert!(!build.target.is_empty());
}

#[test]
fn notices_include_slint_directxtex_and_inter() {
    let notices = notices_text();
    assert!(notices.contains("Slint Royalty-free"));
    assert!(notices.contains("DirectXTex"));
    assert!(notices.contains("SIL OPEN FONT LICENSE"));
}

#[test]
fn notices_do_not_list_removed_code() {
    let notices = notices_text();
    assert!(!notices.contains("System.Net.FtpClient"));
    assert!(!notices.contains("netftp"));
}

struct Ui {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Ui {
    fn start(harness: &mut AppHarness, theme: ThemeChoice) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window);
        ui.show(view.window().window());
        harness.app().set_theme(theme);
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let started = Ui { ui, view, app };
        started.render();
        started
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }

    fn snapshot(&self) -> RgbaImage {
        self.ui.snapshot(self.view.window().window())
    }
}

#[test]
fn about_dialog_renders_with_about_slint() {
    for (theme, name) in THEMES {
        let mut harness = AppHarness::new();
        let ui = Ui::start(&mut harness, theme);
        ui.app.borrow_mut().show_about();
        ui.render();

        let dialog = ui.view.window().global::<DialogModel>();
        assert_eq!(dialog.get_kind(), DialogKind::About);
        assert_eq!(dialog.get_title().as_str(), ABOUT_TITLE);
        assert_eq!(
            dialog.get_about_heading().as_str(),
            "Aurora Asset Editor 2.0.7 (a1b2c3d)"
        );

        let snapshot = ui.snapshot();
        keep_snapshot(&snapshot, &format!("about-{name}"));

        let widget = ElementHandle::find_by_element_id(ui.view.window(), ABOUT_SLINT_ID)
            .next()
            .expect("the About dialog holds the AboutSlint widget");
        let (position, size) = (widget.absolute_position(), widget.size());
        let probe = snapshot.get_pixel(
            (position.x - CARD_PROBE_GAP) as u32,
            (position.y + size.height / 2.0) as u32,
        );
        let card_background = [probe[0], probe[1], probe[2]];
        let mut foreground = HashSet::new();
        for y in position.y as u32..(position.y + size.height) as u32 {
            for x in position.x as u32..(position.x + size.width) as u32 {
                let pixel = snapshot.get_pixel(x, y);
                let colour = [pixel[0], pixel[1], pixel[2]];
                if colour != card_background {
                    foreground.insert(colour);
                }
            }
        }
        assert!(
            foreground.len() >= MIN_FOREGROUND_COLOURS,
            "{name}: AboutSlint drew {} colours besides the card's",
            foreground.len()
        );
    }
}

#[test]
fn about_is_reachable_from_the_build_pill() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness, ThemeChoice::Dark);
    ui.view.window().global::<ShellModel>().invoke_show_about();
    assert!(matches!(ui.app.borrow().modal(), Some(Modal::About)));
}

#[test]
fn open_log_folder_opens_config_dir() {
    let mut harness = AppHarness::new();
    harness.app().show_about();
    harness.app().open_log_folder();
    let config_dir: PathBuf = harness.config_dir().to_path_buf();
    assert_eq!(harness.opened_folders(), vec![config_dir]);
    assert!(matches!(harness.app().modal(), Some(Modal::About)));
}

#[test]
fn open_log_folder_failure_is_shown() {
    let mut harness = AppHarness::new();
    harness.folder_opener().fail_with("no file manager");
    harness.app().show_about();
    harness.app().open_log_folder();
    match harness.app().modal() {
        Some(Modal::Message { title, text }) => {
            assert_eq!(title, OPEN_LOG_FOLDER_FAILED);
            assert!(text.contains("no file manager"), "{text}");
        }
        other => panic!("expected the error dialog, got {other:?}"),
    }
    let log = harness.error_log_text();
    assert!(log.contains(OPEN_LOG_FOLDER_FAILED), "{log}");
    assert!(log.contains("no file manager"), "{log}");
}

#[test]
fn copy_build_info_failure_is_shown() {
    let mut harness = AppHarness::new();
    harness.clipboard().fail_with("clipboard busy");
    harness.app().show_about();
    harness.app().copy_build_info();
    match harness.app().modal() {
        Some(Modal::Message { title, .. }) => assert_eq!(title, COPY_BUILD_INFO_FAILED),
        other => panic!("expected the error dialog, got {other:?}"),
    }
    assert!(harness.error_log_text().contains("clipboard busy"));
}

fn keep_snapshot(image: &RgbaImage, name: &str) {
    if let Some(folder) = std::env::var_os(SNAPSHOT_DIR_VARIABLE) {
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).expect("the snapshot folder can be created");
        image
            .save(folder.join(format!("{name}.png")))
            .expect("the snapshot can be written");
    }
}
