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
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, SharedString};

/// Set to a folder to keep the rendered dialog as PNG files for a look.
const SNAPSHOT_DIR_VARIABLE: &str = "AURORA_UI_SNAPSHOT_DIR";
const ABOUT_SLINT_ID: &str = "AboutDialog::about-slint";
/// The widget draws its logo and text in at least this many colours besides the card's.
const MIN_FOREGROUND_COLOURS: usize = 2;
const CARD_PROBE_GAP: f32 = 4.0;
const DATE_EXAMPLE: &str = "2026-11-07T10:00:00Z";
const MINIMUM_WINDOW: (f32, f32) = (760.0, 500.0);
/// A sideways wheel turn, more than any overhang a line of the notices could have.
const SIDEWAYS_SCROLL: f32 = 200.0;
/// Copy build info, Third-party notices, then the notices list.
const NOTICES_TAB_STOPS: usize = 3;
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

#[test]
fn notices_carry_the_slint_attribution_clause_and_the_inter_copyright() {
    let notices = notices_text();
    assert!(notices.contains("widget in an \"About\" screen or dialog"));
    assert!(notices.contains("Copyright (c) 2016 The Inter Project Authors"));
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

    fn press(&self, key: Key) {
        let text = SharedString::from(key);
        let window = self.view.window().window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
    }

    fn element(&self, id: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(self.view.window(), id)
            .next()
            .unwrap_or_else(|| panic!("the window shows {id}"))
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
fn open_log_folder_creates_a_missing_config_dir() {
    let mut harness = AppHarness::new();
    harness.with_config_dir_not_created();
    harness.app().show_about();
    assert!(!harness.config_dir().exists());

    harness.app().open_log_folder();

    assert!(harness.config_dir().is_dir());
    assert_eq!(
        harness.opened_folders(),
        vec![harness.config_dir().to_path_buf()]
    );
    assert!(matches!(harness.app().modal(), Some(Modal::About)));
}

#[test]
fn open_log_folder_shows_why_the_folder_cannot_be_created() {
    let mut harness = AppHarness::new();
    harness.with_config_dir_not_created();
    std::fs::write(harness.config_dir(), b"a file where the folder belongs")
        .expect("the blocking file can be written");
    harness.app().show_about();

    harness.app().open_log_folder();

    assert!(harness.opened_folders().is_empty());
    match harness.app().modal() {
        Some(Modal::Message { title, text }) => {
            assert_eq!(title, OPEN_LOG_FOLDER_FAILED);
            assert!(!text.is_empty(), "the dialog gives no reason");
        }
        other => panic!("expected the error dialog, got {other:?}"),
    }
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
            assert_eq!(text, "No file manager");
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

#[test]
fn about_failures_give_the_reason_without_repeating_the_title() {
    let mut copy_failure = AppHarness::new();
    copy_failure.clipboard().fail_with("clipboard busy");
    copy_failure.app().show_about();
    copy_failure.app().copy_build_info();

    let mut folder_failure = AppHarness::new();
    folder_failure.folder_opener().fail_with("no file manager");
    folder_failure.app().show_about();
    folder_failure.app().open_log_folder();

    for (harness, expected_text) in [
        (
            &mut copy_failure,
            "Could not use the clipboard: clipboard busy",
        ),
        (&mut folder_failure, "No file manager"),
    ] {
        match harness.app().modal() {
            Some(Modal::Message { title, text }) => {
                assert!(!text.starts_with(title.as_str()), "{title} / {text}");
                assert_eq!(text, expected_text);
            }
            other => panic!("expected the error dialog, got {other:?}"),
        }
    }
}

#[test]
fn copy_build_info_says_copied_inside_the_dialog() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness, ThemeChoice::Dark);
    ui.app.borrow_mut().show_about();
    ui.render();
    ui.element("AboutDialog::copy-button")
        .mock_single_click(PointerEventButton::Left);
    assert!(ui.app.borrow().about_copied());
    assert!(ui.view.window().global::<DialogModel>().get_about_copied());
    assert!(harness.clipboard_text().is_some());
}

#[test]
fn escape_and_enter_close_about() {
    for key in [Key::Escape, Key::Return] {
        let mut harness = AppHarness::new();
        let ui = Ui::start(&mut harness, ThemeChoice::Dark);
        ui.app.borrow_mut().show_about();
        ui.render();
        ui.press(key);
        assert!(ui.app.borrow().modal().is_none());
    }
}

#[test]
fn expanded_notices_fit_the_minimum_window_and_scroll_with_the_keys() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness, ThemeChoice::Dark);
    let window = ui.view.window().window();
    window.set_size(LogicalSize::new(MINIMUM_WINDOW.0, MINIMUM_WINDOW.1));
    ui.app.borrow_mut().show_about();
    ui.app.borrow_mut().toggle_about_notices();
    ui.render();

    let dialog = ui.view.window().global::<DialogModel>();
    assert!(dialog.get_about_notices_expanded());
    assert!(slint::Model::row_count(&dialog.get_about_notices()) > 1000);

    let title = ElementHandle::find_by_accessible_label(ui.view.window(), ABOUT_TITLE)
        .next()
        .expect("the About card is labelled with its title");
    assert!(title.absolute_position().y >= 0.0);
    let ok = ui.element("AboutDialog::ok-button");
    let bottom = ok.absolute_position().y + ok.size().height;
    assert!(bottom <= MINIMUM_WINDOW.1, "the OK row ends at {bottom}");

    let list = ui.element("AboutDialog::notices-keys");
    let (position, size) = (list.absolute_position(), list.size());
    let crop = |image: &RgbaImage| {
        image::imageops::crop_imm(
            image,
            position.x as u32,
            position.y as u32,
            size.width as u32,
            size.height as u32,
        )
        .to_image()
    };
    keep_snapshot(&ui.snapshot(), "about-notices-minimum");
    for _ in 0..NOTICES_TAB_STOPS {
        ui.press(Key::Tab);
    }
    let before = crop(&ui.snapshot());
    ui.press(Key::PageDown);
    let after_page = crop(&ui.snapshot());
    assert!(before.as_raw() != after_page.as_raw(), "PageDown scrolls");
    ui.press(Key::DownArrow);
    let after_line = crop(&ui.snapshot());
    assert!(after_page.as_raw() != after_line.as_raw(), "Down scrolls");
    ui.press(Key::PageUp);
    ui.press(Key::PageUp);
    ui.press(Key::UpArrow);
    assert!(
        crop(&ui.snapshot()).as_raw() == before.as_raw(),
        "Up and PageUp scroll back"
    );
    assert!(ui.app.borrow().modal().is_some());
}

#[test]
fn notices_list_does_not_scroll_sideways() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness, ThemeChoice::Dark);
    ui.app.borrow_mut().show_about();
    ui.app.borrow_mut().toggle_about_notices();
    ui.render();

    let list = ui.element("AboutDialog::notices-keys");
    let (position, size) = (list.absolute_position(), list.size());
    let crop = |image: &RgbaImage| {
        image::imageops::crop_imm(
            image,
            position.x as u32,
            position.y as u32,
            size.width as u32,
            size.height as u32,
        )
        .to_image()
    };
    let before = crop(&ui.snapshot());
    ui.view
        .window()
        .window()
        .dispatch_event(WindowEvent::PointerScrolled {
            position: LogicalPosition::new(
                position.x + size.width / 2.0,
                position.y + size.height / 2.0,
            ),
            delta_x: -SIDEWAYS_SCROLL,
            delta_y: 0.0,
        });
    assert!(
        crop(&ui.snapshot()).as_raw() == before.as_raw(),
        "the notices moved sideways"
    );
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
