use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::{self, ThreadId};

use aurora_asset_editor::app::Menu;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{
    AppHarness, ImageReport, ImageStatus, ReferenceMode, ReferenceStore, Scene, Screen, ScreenArea,
    ScreenFilterOnCi, ScreenshotSuite, SuiteReport, read_png, write_opaque_png,
};
use image::{Rgba, RgbaImage};
use tempfile::TempDir;

const AREA_TITLE: &str = "Menus under test";
const SCREEN_A_DESCRIPTION: &str = "The gear menu open";
const ESCAPED_DESCRIPTION: &str = "a < b & c";
const UPDATE_COMMAND: &str = "cargo xtask ui-screenshots update";
const MAGENTA: Rgba<u8> = Rgba([0xff, 0x00, 0xff, 0xff]);

static STAGING_THREADS: Mutex<Vec<ThreadId>> = Mutex::new(Vec::new());
/// Files a stage looks for, and whether each was on disk while the screen was staged.
static WATCHED_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static WATCHED_FILES_SEEN: Mutex<Vec<bool>> = Mutex::new(Vec::new());

fn menu_scene(theme: ThemeChoice, menu: Menu) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.toggle_menu(Some(menu)));
    scene
}

fn stage_gear_menu(theme: ThemeChoice) -> Scene {
    menu_scene(theme, Menu::Gear)
}

fn stage_open_menu(theme: ThemeChoice) -> Scene {
    menu_scene(theme, Menu::Open)
}

fn stage_panicking(_theme: ThemeChoice) -> Scene {
    panic!("boom")
}

fn record_thread() {
    STAGING_THREADS
        .lock()
        .expect("no thread panicked while holding the list")
        .push(thread::current().id());
}

fn stage_gear_menu_recording_thread(theme: ThemeChoice) -> Scene {
    record_thread();
    stage_gear_menu(theme)
}

fn stage_open_menu_recording_thread(theme: ThemeChoice) -> Scene {
    record_thread();
    stage_open_menu(theme)
}

fn stage_gear_menu_watching_files(theme: ThemeChoice) -> Scene {
    let watched = WATCHED_FILES
        .lock()
        .expect("no thread panicked while holding the list");
    WATCHED_FILES_SEEN
        .lock()
        .expect("no thread panicked while holding the list")
        .extend(watched.iter().map(|path| path.exists()));
    stage_gear_menu(theme)
}

const SCREEN_A: Screen = Screen {
    name: "a",
    description: SCREEN_A_DESCRIPTION,
    stage: stage_gear_menu,
};
const SCREEN_B: Screen = Screen {
    name: "b",
    description: "The open menu open",
    stage: stage_open_menu,
};
const TWO_SCREENS: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[SCREEN_A, SCREEN_B],
}];
const RECORDING_SCREENS: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[
        Screen {
            name: "a",
            description: SCREEN_A_DESCRIPTION,
            stage: stage_gear_menu_recording_thread,
        },
        Screen {
            name: "b",
            description: "The open menu open",
            stage: stage_open_menu_recording_thread,
        },
    ],
}];
const ONE_BROKEN_SCREEN: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[
        Screen {
            name: "broken",
            description: "Staging panics",
            stage: stage_panicking,
        },
        SCREEN_A,
        SCREEN_B,
    ],
}];
const SCREEN_WITH_MARKUP: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[Screen {
        name: "a",
        description: ESCAPED_DESCRIPTION,
        stage: stage_gear_menu,
    }],
}];

/// A references folder and an output folder inside one temporary folder.
struct SuiteFolders {
    _folder: TempDir,
    references: PathBuf,
    output: PathBuf,
}

impl SuiteFolders {
    fn new() -> SuiteFolders {
        let folder = tempfile::tempdir().expect("a temporary folder is needed for the suite");
        SuiteFolders {
            references: folder.path().join("references"),
            output: folder.path().join("output"),
            _folder: folder,
        }
    }

    fn store(&self) -> ReferenceStore {
        ReferenceStore::new(self.references.clone(), self.output.clone())
    }

    fn run(&self, mode: ReferenceMode, only: Option<&str>, areas: &[ScreenArea]) -> SuiteReport {
        ScreenshotSuite::new(self.store(), mode, only.map(str::to_owned)).run(areas)
    }

    fn write_png(&self, path: &Path) {
        let image = RgbaImage::from_pixel(2, 2, MAGENTA);
        write_opaque_png(path, &image).expect("the test image is written");
    }

    /// Changes two pixels of an approved reference, so the next check finds "2 pixels differ".
    fn change_reference(&self, image: &str) {
        let path = self.store().reference_path(image);
        let mut reference = read_png(&path).expect("the reference was written by an update run");
        reference.put_pixel(0, 0, MAGENTA);
        reference.put_pixel(1, 0, MAGENTA);
        write_opaque_png(&path, &reference).expect("the changed reference is written");
    }

    fn sheet(&self, report: &SuiteReport) -> String {
        let path = report
            .sheet
            .as_ref()
            .expect("the run wrote a contact sheet");
        assert_eq!(path, &self.output.join("index.html"));
        fs::read_to_string(path).expect("the contact sheet is readable")
    }
}

fn image_names(report: &SuiteReport) -> Vec<&str> {
    report.images().map(|image| image.image.as_str()).collect()
}

fn image<'a>(report: &'a SuiteReport, name: &str) -> &'a ImageReport {
    report
        .images()
        .find(|image| image.image == name)
        .unwrap_or_else(|| panic!("the report has the image {name}"))
}

#[test]
fn every_screen_is_checked_in_both_themes() {
    let folders = SuiteFolders::new();

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    assert_eq!(
        image_names(&report),
        ["a-dark", "a-light", "b-dark", "b-light"]
    );
    for image in report.images() {
        assert_eq!(image.status, ImageStatus::Created, "{}", image.image);
    }
    assert_eq!(report.areas.len(), 1);
    assert_eq!(report.areas[0].title, AREA_TITLE);
    assert!(report.passed(), "{report}");
}

#[test]
fn each_image_is_staged_on_its_own_thread() {
    let folders = SuiteFolders::new();

    folders.run(ReferenceMode::Update, None, RECORDING_SCREENS);

    let threads = STAGING_THREADS
        .lock()
        .expect("no thread panicked while holding the list");
    let distinct: BTreeSet<String> = threads.iter().map(|id| format!("{id:?}")).collect();
    assert_eq!(threads.len(), 4);
    assert_eq!(distinct.len(), 4, "{threads:?}");
    assert!(!threads.contains(&thread::current().id()));
}

#[test]
fn staging_panic_fails_the_run_and_names_the_image() {
    let folders = SuiteFolders::new();
    let stale_actual = folders.store().actual_path("broken-dark");
    folders.write_png(&stale_actual);

    let report = folders.run(ReferenceMode::Update, None, ONE_BROKEN_SCREEN);

    for name in ["broken-dark", "broken-light"] {
        match &image(&report, name).status {
            ImageStatus::Error(message) => {
                assert!(message.contains("staging or capture panicked"), "{message}");
                assert!(message.contains("boom"), "{message}");
            }
            other => panic!("{name} is an error, not {other:?}"),
        }
    }
    for name in ["a-dark", "a-light", "b-dark", "b-light"] {
        assert_eq!(image(&report, name).status, ImageStatus::Created, "{name}");
    }
    assert!(!report.passed());
    assert!(!stale_actual.exists(), "the stale screenshot is removed");
    let text = report.to_string();
    assert!(text.contains("broken-dark"), "{text}");
    assert!(text.contains("boom"), "{text}");
}

#[test]
fn orphan_reference_fails_in_check_mode() {
    let folders = SuiteFolders::new();
    folders.run(ReferenceMode::Update, None, TWO_SCREENS);
    folders.write_png(&folders.store().reference_path("extra"));

    let report = folders.run(ReferenceMode::Check, None, TWO_SCREENS);

    for image in report.images() {
        assert_eq!(image.status, ImageStatus::Matches, "{}", image.image);
    }
    assert_eq!(report.orphans, ["extra"]);
    assert!(!report.passed());
    assert!(report.to_string().contains("extra"), "{report}");
}

#[test]
fn update_mode_removes_orphans() {
    let folders = SuiteFolders::new();
    let orphan = folders.store().reference_path("extra");
    folders.write_png(&orphan);

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    assert_eq!(report.removed_orphans, ["extra"]);
    assert!(report.orphans.is_empty());
    assert!(!orphan.exists());
    assert!(report.passed(), "{report}");
}

#[test]
fn filtered_run_is_reported_as_filtered() {
    let folders = SuiteFolders::new();
    let orphan = folders.store().reference_path("extra");
    let other_screen_actual = folders.store().actual_path("a-dark");
    folders.write_png(&orphan);
    folders.write_png(&other_screen_actual);

    let report = folders.run(ReferenceMode::Update, Some("b"), TWO_SCREENS);

    assert_eq!(image_names(&report), ["b-dark", "b-light"]);
    assert!(report.orphans.is_empty());
    assert!(report.removed_orphans.is_empty());
    assert!(orphan.exists(), "a filtered run leaves orphans alone");
    assert!(
        other_screen_actual.exists(),
        "a filtered run leaves the outputs of other screens alone"
    );
    assert_eq!(report.filter.as_deref(), Some("b"));
    let text = report.to_string();
    assert!(
        text.contains("filtered to screens containing \"b\""),
        "{text}"
    );
}

#[test]
fn filter_matching_no_screen_fails_the_run() {
    let folders = SuiteFolders::new();

    let report = folders.run(ReferenceMode::Update, Some("nothing"), TWO_SCREENS);

    assert_eq!(report.images().count(), 0);
    assert!(!report.passed());
    assert!(report.to_string().contains("nothing"), "{report}");
}

#[test]
fn full_run_clears_outputs_of_screens_that_no_longer_exist() {
    let folders = SuiteFolders::new();
    let store = folders.store();
    let retired = [
        store.actual_path("gone-dark"),
        store.diff_path("gone-dark"),
        store.reference_copy_path("gone-dark"),
    ];
    for path in &retired {
        folders.write_png(path);
    }

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    for path in &retired {
        assert!(!path.exists(), "{} is removed", path.display());
    }
    assert!(report.passed(), "{report}");
}

#[test]
fn report_names_the_failures_and_the_update_command() {
    let folders = SuiteFolders::new();
    folders.run(ReferenceMode::Update, None, TWO_SCREENS);
    folders.change_reference("a-dark");

    let report = folders.run(ReferenceMode::Check, None, TWO_SCREENS);

    assert!(matches!(
        image(&report, "a-dark").status,
        ImageStatus::Differs(_)
    ));
    assert!(!report.passed());
    let text = report.to_string();
    let store = folders.store();
    assert!(text.contains("a-dark"), "{text}");
    assert!(text.contains("2 pixels differ"), "{text}");
    assert!(
        text.contains(&store.diff_path("a-dark").display().to_string()),
        "{text}"
    );
    assert!(
        text.contains(&store.actual_path("a-dark").display().to_string()),
        "{text}"
    );
    assert!(text.contains(UPDATE_COMMAND), "{text}");
    let written =
        fs::read_to_string(folders.output.join("report.txt")).expect("the run wrote report.txt");
    assert_eq!(written, text);
}

#[test]
fn contact_sheet_lists_every_image_with_its_description() {
    let folders = SuiteFolders::new();

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    let sheet = folders.sheet(&report);
    assert!(sheet.contains("actual/a-dark.png"), "{sheet}");
    assert!(sheet.contains("actual/a-light.png"), "{sheet}");
    assert!(sheet.contains(SCREEN_A_DESCRIPTION), "{sheet}");
    assert!(sheet.contains(AREA_TITLE), "{sheet}");
}

#[test]
fn contact_sheet_shows_reference_actual_and_diff_for_a_failure() {
    let folders = SuiteFolders::new();
    folders.run(ReferenceMode::Update, None, TWO_SCREENS);
    folders.change_reference("a-dark");

    let report = folders.run(ReferenceMode::Check, None, TWO_SCREENS);

    let sheet = folders.sheet(&report);
    assert!(sheet.contains("reference/a-dark.png"), "{sheet}");
    assert!(sheet.contains("actual/a-dark.png"), "{sheet}");
    assert!(sheet.contains("diff/a-dark.png"), "{sheet}");
    assert!(sheet.contains(">differs</span>"), "{sheet}");
    assert!(sheet.contains("Needs a look"), "{sheet}");
    assert!(sheet.contains("href=\"#a-dark\""), "{sheet}");
    assert!(!sheet.contains("diff/a-light.png"), "{sheet}");
}

#[test]
fn contact_sheet_marks_updated_and_new_images() {
    let folders = SuiteFolders::new();
    folders.run(ReferenceMode::Update, None, TWO_SCREENS);
    folders.change_reference("a-dark");
    fs::remove_file(folders.store().reference_path("b-dark")).expect("the reference exists");

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    assert!(matches!(
        image(&report, "a-dark").status,
        ImageStatus::Replaced(_)
    ));
    assert_eq!(image(&report, "b-dark").status, ImageStatus::Created);
    let sheet = folders.sheet(&report);
    assert!(sheet.contains(">updated</span>"), "{sheet}");
    assert!(sheet.contains(">new</span>"), "{sheet}");
}

#[test]
fn contact_sheet_escapes_text() {
    let folders = SuiteFolders::new();

    let report = folders.run(ReferenceMode::Update, None, SCREEN_WITH_MARKUP);

    let sheet = folders.sheet(&report);
    assert!(sheet.contains("a &lt; b &amp; c"), "{sheet}");
    assert!(!sheet.contains(ESCAPED_DESCRIPTION), "{sheet}");
}

#[test]
fn screen_name_used_twice_fails_the_run() {
    const REPEATED_NAME: &[ScreenArea] = &[ScreenArea {
        title: AREA_TITLE,
        screens: &[SCREEN_A, SCREEN_A],
    }];
    let folders = SuiteFolders::new();

    let report = folders.run(ReferenceMode::Update, None, REPEATED_NAME);

    assert!(!report.passed());
    assert!(
        report
            .problems
            .iter()
            .any(|problem| problem.contains("a-dark") && problem.contains("more than one screen")),
        "{report}"
    );
    let sheet = folders.sheet(&report);
    assert_eq!(sheet.matches("id=\"a-dark\"").count(), 1, "{sheet}");
}

#[test]
fn report_text_that_cannot_be_written_fails_the_sheet_too() {
    let folders = SuiteFolders::new();
    fs::create_dir_all(folders.output.join("report.txt"))
        .expect("a folder in place of report.txt is created");

    let report = folders.run(ReferenceMode::Update, None, TWO_SCREENS);

    assert!(!report.passed());
    assert!(
        report
            .problems
            .iter()
            .any(|problem| problem.contains("Could not write") && problem.contains("report.txt")),
        "{report}"
    );
    let sheet = folders.sheet(&report);
    assert!(!sheet.contains(">Passed<"), "{sheet}");
    assert!(sheet.contains(">Failed<"), "{sheet}");
    assert!(sheet.contains("report.txt"), "{sheet}");
}

#[test]
fn screen_filter_comes_from_its_value() {
    assert_eq!(ScreenshotSuite::filter_from_values(None, None), Ok(None));
    assert_eq!(
        ScreenshotSuite::filter_from_values(Some(""), None),
        Ok(None)
    );
    assert_eq!(
        ScreenshotSuite::filter_from_values(Some("cover"), None),
        Ok(Some("cover".to_owned()))
    );
    assert_eq!(
        ScreenshotSuite::filter_from_values(Some("cover"), Some("")),
        Ok(Some("cover".to_owned()))
    );
    assert_eq!(
        ScreenshotSuite::filter_from_values(Some(""), Some("true")),
        Ok(None)
    );
}

#[test]
fn screen_filter_is_refused_on_ci() {
    let refused = ScreenshotSuite::filter_from_values(Some("cover"), Some("true"));

    assert_eq!(refused, Err(ScreenFilterOnCi));
    assert_eq!(
        ScreenFilterOnCi.to_string(),
        "A screen filter is not allowed on CI; unset AURORA_UI_SCREENS"
    );
}

#[test]
fn previous_sheet_and_report_are_removed_before_staging() {
    const WATCHING_SCREEN: &[ScreenArea] = &[ScreenArea {
        title: AREA_TITLE,
        screens: &[Screen {
            name: "a",
            description: SCREEN_A_DESCRIPTION,
            stage: stage_gear_menu_watching_files,
        }],
    }];
    const STALE_MARKER: &str = "stale result of an earlier run";
    let folders = SuiteFolders::new();
    let previous = [
        folders.output.join("index.html"),
        folders.output.join("report.txt"),
    ];
    fs::create_dir_all(&folders.output).expect("the output folder is created");
    for path in &previous {
        fs::write(path, STALE_MARKER).expect("the stale result is written");
    }
    *WATCHED_FILES
        .lock()
        .expect("no thread panicked while holding the list") = previous.to_vec();

    let report = folders.run(ReferenceMode::Update, None, WATCHING_SCREEN);

    let seen = WATCHED_FILES_SEEN
        .lock()
        .expect("no thread panicked while holding the list");
    assert_eq!(
        *seen,
        [false, false, false, false],
        "index.html, report.txt per theme"
    );
    assert!(report.passed(), "{report}");
    assert!(!folders.sheet(&report).contains(STALE_MARKER));
}
