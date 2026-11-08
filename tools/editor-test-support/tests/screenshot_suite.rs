use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::{self, ThreadId};

use aurora_asset_editor::app::Menu;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{
    AppHarness, BaselineSource, DEFAULT_LABEL, ImageChange, ImageReport, RunManifest, Scene,
    Screen, ScreenArea, ScreenFilterOnCi, ScreenshotFolders, ScreenshotSuite, SuiteReport,
};
use tempfile::TempDir;

const PLATFORM: &str = "test-platform";
const OTHER_PLATFORM: &str = "other-platform";
const AREA_TITLE: &str = "Menus under test";
const SCREEN_A_DESCRIPTION: &str = "The gear menu open";
const ESCAPED_DESCRIPTION: &str = "a < b & c";
const ALL_IMAGES: [&str; 4] = ["a-dark", "a-light", "b-dark", "b-light"];

static STAGING_THREADS: Mutex<Vec<ThreadId>> = Mutex::new(Vec::new());
/// Where the stage of `REPORT_BLOCKING_SCREENS` puts a folder, so `report.txt` cannot be written.
static REPORT_BLOCKER: Mutex<Option<PathBuf>> = Mutex::new(None);

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

fn stage_gear_menu_blocking_report(theme: ThemeChoice) -> Scene {
    let blocker = REPORT_BLOCKER
        .lock()
        .expect("no thread panicked while holding the path");
    let path = blocker.as_ref().expect("the test sets the blocked path");
    fs::create_dir_all(path).expect("a folder in place of report.txt is created");
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
/// `TWO_SCREENS` after screen a changed: it now shows the open menu.
const SCREEN_A_CHANGED: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[
        Screen {
            name: "a",
            description: SCREEN_A_DESCRIPTION,
            stage: stage_open_menu,
        },
        SCREEN_B,
    ],
}];
const ONLY_SCREEN_A: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[SCREEN_A],
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
const REPORT_BLOCKING_SCREENS: &[ScreenArea] = &[ScreenArea {
    title: AREA_TITLE,
    screens: &[Screen {
        name: "a",
        description: SCREEN_A_DESCRIPTION,
        stage: stage_gear_menu_blocking_report,
    }],
}];

/// The screenshot folders of the platform `test-platform` inside one temporary folder.
struct SuiteFolders {
    temp: TempDir,
}

impl SuiteFolders {
    fn new() -> SuiteFolders {
        SuiteFolders {
            temp: tempfile::tempdir().expect("a temporary folder is needed for the suite"),
        }
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn folders(&self) -> ScreenshotFolders {
        ScreenshotFolders::new(self.root().to_path_buf(), PLATFORM.to_owned())
    }

    fn current(&self) -> PathBuf {
        self.root().join(PLATFORM)
    }

    fn previous(&self) -> PathBuf {
        self.root().join("previous").join(PLATFORM)
    }

    fn run(&self, areas: &[ScreenArea]) -> SuiteReport {
        self.run_labelled(DEFAULT_LABEL, None, areas)
    }

    fn run_labelled(&self, label: &str, only: Option<&str>, areas: &[ScreenArea]) -> SuiteReport {
        self.run_with(BaselineSource::PreviousRun, label, only, areas)
    }

    fn run_against(&self, baseline: &Path, areas: &[ScreenArea]) -> SuiteReport {
        let source = BaselineSource::Folder(baseline.to_path_buf());
        self.run_with(source, DEFAULT_LABEL, None, areas)
    }

    fn run_with(
        &self,
        baseline: BaselineSource,
        label: &str,
        only: Option<&str>,
        areas: &[ScreenArea],
    ) -> SuiteReport {
        let suite = ScreenshotSuite::new(
            self.folders(),
            baseline,
            only.map(str::to_owned),
            label.to_owned(),
        );
        suite.run(areas)
    }

    fn sheet(&self, report: &SuiteReport) -> String {
        let path = report
            .sheet
            .as_ref()
            .expect("the run wrote a contact sheet");
        assert_eq!(path, &self.current().join("index.html"));
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

fn baseline_label(report: &SuiteReport) -> &str {
    report
        .baseline
        .as_ref()
        .map(|manifest| manifest.label.as_str())
        .expect("the run had a baseline")
}

fn has_problem(report: &SuiteReport, wanted: impl Fn(&str) -> bool) -> bool {
    report.problems.iter().any(|problem| wanted(problem))
}

/// `second` appears apart from `first`, so the text names both even when `first` contains
/// `second`.
fn names_both(text: &str, first: &str, second: &str) -> bool {
    text.split_once(first)
        .is_some_and(|(before, after)| before.contains(second) || after.contains(second))
}

fn has_note(report: &SuiteReport, text: &str) -> bool {
    report.notes.iter().any(|note| note.contains(text))
}

fn assert_every_image_is(report: &SuiteReport, change: &ImageChange) {
    for image in report.images() {
        assert_eq!(&image.change, change, "{}", image.image);
    }
}

/// A finished run of another platform: its manifest and nothing else.
fn finished_run_of_other_platform(folder: &Path) {
    let manifest = RunManifest {
        platform: OTHER_PLATFORM.to_owned(),
        label: DEFAULT_LABEL.to_owned(),
        rendered: "2026-11-01 10:00 UTC".to_owned(),
        filter: None,
    };
    manifest
        .write(folder)
        .expect("the other platform's manifest is written");
}

#[test]
fn first_run_has_no_baseline_and_every_image_is_new() {
    let folders = SuiteFolders::new();

    let report = folders.run(TWO_SCREENS);

    assert_eq!(image_names(&report), ALL_IMAGES);
    assert_every_image_is(&report, &ImageChange::New);
    assert!(report.baseline.is_none());
    assert!(
        has_note(&report, "no earlier finished local run"),
        "{report}"
    );
    assert_eq!(report.areas.len(), 1);
    assert_eq!(report.areas[0].title, AREA_TITLE);
    assert!(report.passed(), "{report}");
}

#[test]
fn second_run_compares_with_the_first() {
    let folders = SuiteFolders::new();
    folders.run(TWO_SCREENS);

    let report = folders.run(TWO_SCREENS);

    assert_every_image_is(&report, &ImageChange::Unchanged);
    assert_eq!(baseline_label(&report), DEFAULT_LABEL);
    assert!(folders.previous().join("manifest.txt").is_file());
    assert!(report.passed(), "{report}");
}

#[test]
fn changed_screen_is_reported_and_does_not_fail() {
    let folders = SuiteFolders::new();
    folders.run(TWO_SCREENS);

    let report = folders.run(SCREEN_A_CHANGED);

    for name in ["a-dark", "a-light"] {
        assert!(
            matches!(image(&report, name).change, ImageChange::Changed(_)),
            "{name} changed: {report}"
        );
    }
    for name in ["b-dark", "b-light"] {
        assert_eq!(
            image(&report, name).change,
            ImageChange::Unchanged,
            "{name}"
        );
    }
    assert!(report.passed(), "{report}");
    assert!(folders.current().join("diff").join("a-dark.png").is_file());
    let text = report.to_string();
    assert!(text.contains("2 changed"), "{text}");
    assert!(text.contains("Changes do not fail the run"), "{text}");
}

#[test]
fn removed_screen_is_listed_with_its_baseline_image() {
    let folders = SuiteFolders::new();
    folders.run(TWO_SCREENS);

    let report = folders.run(ONLY_SCREEN_A);

    assert_eq!(report.removed, ["b-dark", "b-light"]);
    assert!(
        folders
            .current()
            .join("baseline")
            .join("b-dark.png")
            .is_file()
    );
    assert!(report.passed(), "{report}");
}

#[test]
fn each_image_is_staged_on_its_own_thread() {
    let folders = SuiteFolders::new();

    folders.run(RECORDING_SCREENS);

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

    let report = folders.run(ONE_BROKEN_SCREEN);

    for name in ["broken-dark", "broken-light"] {
        match &image(&report, name).change {
            ImageChange::Error(message) => {
                assert!(message.contains("staging or capture panicked"), "{message}");
                assert!(message.contains("boom"), "{message}");
            }
            other => panic!("{name} is an error, not {other:?}"),
        }
    }
    for name in ALL_IMAGES {
        assert_eq!(image(&report, name).change, ImageChange::New, "{name}");
    }
    assert!(!report.passed());
    assert!(folders.current().join("manifest.txt").is_file());
    let text = report.to_string();
    assert!(text.contains("broken-dark"), "{text}");
    assert!(text.contains("boom"), "{text}");
}

#[test]
fn each_run_starts_from_an_empty_output_folder() {
    let folders = SuiteFolders::new();
    let stale = [
        folders.current().join("images").join("stale-dark.png"),
        folders.current().join("diff").join("a-dark.png"),
    ];
    for path in &stale {
        fs::create_dir_all(path.parent().expect("the stale file has a folder"))
            .expect("the stale file's folder is created");
        fs::write(path, b"stale").expect("the stale file is written");
    }

    let report = folders.run(TWO_SCREENS);

    for path in &stale {
        assert!(!path.exists(), "{} is gone", path.display());
    }
    assert!(report.passed(), "{report}");
}

#[test]
fn unfinished_local_run_is_never_the_baseline() {
    let folders = SuiteFolders::new();
    folders.run_labelled("first", None, TWO_SCREENS);
    folders.run_labelled("second", None, TWO_SCREENS);
    fs::remove_file(folders.current().join("manifest.txt"))
        .expect("the second run wrote its manifest");

    let report = folders.run(TWO_SCREENS);

    assert_eq!(baseline_label(&report), "first");
    assert!(has_note(&report, "did not finish"), "{report}");
    assert!(report.passed(), "{report}");
}

#[test]
fn filtered_run_never_becomes_the_baseline() {
    let folders = SuiteFolders::new();
    folders.run_labelled("full", None, TWO_SCREENS);
    folders.run_labelled("filtered", Some("b"), TWO_SCREENS);

    let report = folders.run_labelled("again", None, TWO_SCREENS);

    assert_eq!(baseline_label(&report), "full");
    assert_every_image_is(&report, &ImageChange::Unchanged);
    assert!(report.passed(), "{report}");
}

#[test]
fn filtered_run_is_reported_as_filtered() {
    let folders = SuiteFolders::new();
    folders.run(TWO_SCREENS);

    let report = folders.run_labelled(DEFAULT_LABEL, Some("b"), TWO_SCREENS);

    assert_eq!(image_names(&report), ["b-dark", "b-light"]);
    assert!(report.removed.is_empty(), "{report}");
    assert_eq!(report.filter.as_deref(), Some("b"));
    let text = report.to_string();
    assert!(
        text.contains("filtered to screens containing \"b\""),
        "{text}"
    );
}

#[test]
fn baseline_from_another_platform_fails_the_run() {
    let folders = SuiteFolders::new();
    let other = folders.root().join("other-run");
    finished_run_of_other_platform(&other);

    let report = folders.run_against(&other, TWO_SCREENS);

    assert!(!report.passed());
    assert!(
        has_problem(&report, |problem| problem.contains(OTHER_PLATFORM)
            && problem.contains(PLATFORM)),
        "{report}"
    );
    assert_every_image_is(&report, &ImageChange::New);
    assert!(
        folders
            .current()
            .join("images")
            .join("a-dark.png")
            .is_file()
    );
}

#[test]
fn missing_baseline_folder_fails_the_run() {
    let folders = SuiteFolders::new();
    let nowhere = folders.root().join("nowhere");

    let report = folders.run_against(&nowhere, TWO_SCREENS);

    assert!(!report.passed());
    let named = nowhere.display().to_string();
    assert!(
        has_problem(&report, |problem| problem.contains(&named)),
        "{report}"
    );
}

#[test]
fn relative_baseline_fails_the_run() {
    let folders = SuiteFolders::new();

    let report = folders.run_against(Path::new("relative/baseline"), TWO_SCREENS);

    assert!(!report.passed());
    assert!(
        has_problem(&report, |problem| problem.contains("AURORA_UI_BASELINE")
            && problem.contains("must be absolute")),
        "{report}"
    );
    assert_every_image_is(&report, &ImageChange::New);
    assert!(
        folders
            .current()
            .join("images")
            .join("a-dark.png")
            .is_file()
    );
}

#[test]
fn baseline_inside_the_run_folder_fails_the_run() {
    for subfolder in [None, Some("images")] {
        let folders = SuiteFolders::new();
        let baseline = match subfolder {
            Some(name) => folders.current().join(name),
            None => folders.current(),
        };
        folders.run_labelled("first", None, TWO_SCREENS);

        let report = folders.run_against(&baseline, TWO_SCREENS);

        assert!(!report.passed());
        let baseline_named = baseline.display().to_string();
        let current_named = folders.current().display().to_string();
        assert!(
            has_problem(&report, |problem| names_both(
                problem,
                &baseline_named,
                &current_named
            )),
            "{report}"
        );
        let kept = RunManifest::read(&folders.previous()).expect("the first run was kept");
        assert_eq!(kept.label, "first");
        for name in ALL_IMAGES {
            let path = folders
                .previous()
                .join("images")
                .join(format!("{name}.png"));
            assert!(path.is_file(), "{} was kept", path.display());
        }
    }
}

#[test]
fn unreadable_baseline_image_does_not_fail_the_run() {
    let folders = SuiteFolders::new();
    folders.run(TWO_SCREENS);
    fs::write(
        folders.current().join("images").join("a-dark.png"),
        b"not a png",
    )
    .expect("the first run's image is overwritten");

    let report = folders.run(TWO_SCREENS);

    assert!(
        matches!(
            image(&report, "a-dark").change,
            ImageChange::BaselineUnreadable(_)
        ),
        "{report}"
    );
    assert!(report.passed(), "{report}");
}

#[test]
fn baseline_comes_from_its_value() {
    assert_eq!(
        ScreenshotSuite::baseline_from_value(None),
        BaselineSource::PreviousRun
    );
    assert_eq!(
        ScreenshotSuite::baseline_from_value(Some("")),
        BaselineSource::PreviousRun
    );
    assert_eq!(
        ScreenshotSuite::baseline_from_value(Some("x")),
        BaselineSource::Folder(PathBuf::from("x"))
    );
}

#[test]
fn label_comes_from_its_value() {
    assert_eq!(ScreenshotSuite::label_from_value(None), "local run");
    assert_eq!(ScreenshotSuite::label_from_value(Some("")), "local run");
    assert_eq!(
        ScreenshotSuite::label_from_value(Some("v2.0.12 (abc1234)")),
        "v2.0.12 (abc1234)"
    );
}

#[test]
fn finished_run_writes_its_manifest() {
    let folders = SuiteFolders::new();

    folders.run_labelled("v2.0.12 (abc1234)", None, TWO_SCREENS);

    let manifest = RunManifest::read(&folders.current()).expect("the run wrote its manifest");
    assert_eq!(manifest.platform, PLATFORM);
    assert_eq!(manifest.label, "v2.0.12 (abc1234)");
    assert!(manifest.rendered.ends_with(" UTC"), "{}", manifest.rendered);
    assert_eq!(manifest.filter, None);

    folders.run_labelled(DEFAULT_LABEL, Some("b"), TWO_SCREENS);

    let filtered = RunManifest::read(&folders.current()).expect("the run wrote its manifest");
    assert_eq!(filtered.filter.as_deref(), Some("b"));
}

#[test]
fn filter_matching_no_screen_fails_the_run() {
    let folders = SuiteFolders::new();

    let report = folders.run_labelled(DEFAULT_LABEL, Some("nothing"), TWO_SCREENS);

    assert_eq!(report.images().count(), 0);
    assert!(!report.passed());
    assert!(report.to_string().contains("nothing"), "{report}");
}

#[test]
fn screen_name_used_twice_fails_the_run() {
    const REPEATED_NAME: &[ScreenArea] = &[ScreenArea {
        title: AREA_TITLE,
        screens: &[SCREEN_A, SCREEN_A],
    }];
    let folders = SuiteFolders::new();

    let report = folders.run(REPEATED_NAME);

    assert!(!report.passed());
    assert!(
        has_problem(&report, |problem| problem.contains("a-dark")
            && problem.contains("more than one screen")),
        "{report}"
    );
    let sheet = folders.sheet(&report);
    assert_eq!(sheet.matches("id=\"a-dark\"").count(), 1, "{sheet}");
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
fn report_text_that_cannot_be_written_fails_the_sheet_too() {
    let folders = SuiteFolders::new();
    *REPORT_BLOCKER
        .lock()
        .expect("no thread panicked while holding the path") =
        Some(folders.current().join("report.txt"));

    let report = folders.run(REPORT_BLOCKING_SCREENS);

    assert!(!report.passed());
    assert!(
        has_problem(&report, |problem| problem.contains("Could not write")
            && problem.contains("report.txt")),
        "{report}"
    );
    let sheet = folders.sheet(&report);
    assert!(!sheet.contains(">Passed<"), "{sheet}");
    assert!(sheet.contains(">Failed<"), "{sheet}");
    assert!(sheet.contains("report.txt"), "{sheet}");
}

#[test]
fn contact_sheet_lists_every_image_with_its_description() {
    let folders = SuiteFolders::new();

    let report = folders.run(TWO_SCREENS);

    let sheet = folders.sheet(&report);
    assert!(sheet.contains("images/a-dark.png"), "{sheet}");
    assert!(sheet.contains("images/a-light.png"), "{sheet}");
    assert!(sheet.contains(SCREEN_A_DESCRIPTION), "{sheet}");
    assert!(sheet.contains(AREA_TITLE), "{sheet}");
}

#[test]
fn contact_sheet_escapes_text() {
    let folders = SuiteFolders::new();

    let report = folders.run(SCREEN_WITH_MARKUP);

    let sheet = folders.sheet(&report);
    assert!(sheet.contains("a &lt; b &amp; c"), "{sheet}");
    assert!(!sheet.contains(ESCAPED_DESCRIPTION), "{sheet}");
}
