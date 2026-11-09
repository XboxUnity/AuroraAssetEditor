use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aurora_asset::{AssetKind, AssetSlot, Compression};
use aurora_asset_editor::app::StartupProblem;
use aurora_asset_editor::platform::{FileDialogs, OpenRequest, SaveRequest};
use aurora_asset_editor::smoke_test::{
    SMOKE_TEST_NO_OTHER_DIALOGS, SMOKE_TEST_TIMEOUT, SmokeStep, SmokeTestDialogs, SmokeTestDriver,
    smoke_timeout_text,
};
use aurora_image::ImageTarget;
use editor_test_support::{AppHarness, asset_file_with, placeholder};
use tempfile::TempDir;

/// Far more steps than a load and a save take; a driver still waiting after them is stuck.
const MAX_STEPS: usize = 20;
const STEP_TIME: Duration = Duration::from_millis(100);

fn write_cover_asset(folder: &Path) -> PathBuf {
    let path = folder.join("GC.asset");
    let mut file = asset_file_with(&[(
        AssetSlot::Cover,
        placeholder(ImageTarget::Cover),
        Compression::Dxt,
    )]);
    fs::write(&path, file.to_bytes()).unwrap();
    path
}

fn request(title: &str) -> OpenRequest {
    OpenRequest {
        title: title.to_owned(),
        filters: Vec::new(),
        default_filter: 0,
        default_name: None,
    }
}

/// Steps the driver as the app's timer does, running the jobs each step submits, until it stops waiting.
fn drive(harness: &mut AppHarness, driver: &mut SmokeTestDriver) -> SmokeStep {
    for count in 0..MAX_STEPS {
        let elapsed = STEP_TIME * u32::try_from(count).unwrap();
        let step = driver.step(&mut harness.app(), elapsed);
        if step != SmokeStep::Waiting {
            return step;
        }
        harness.run_jobs();
    }
    panic!("the driver was still waiting after {MAX_STEPS} steps");
}

/// Steps the driver through opening the asset and saving it; the next step checks the saved file.
fn step_until_saved(harness: &mut AppHarness, driver: &mut SmokeTestDriver) {
    assert_eq!(
        driver.step(&mut harness.app(), Duration::ZERO),
        SmokeStep::Waiting
    );
    harness.run_jobs();
    assert_eq!(
        driver.step(&mut harness.app(), STEP_TIME),
        SmokeStep::Waiting
    );
}

#[test]
fn smoke_dialogs_answer_save_with_the_output_and_refuse_open() {
    let output = PathBuf::from("out.asset");
    let mut dialogs = SmokeTestDialogs::new(output.clone());
    let save = SaveRequest {
        title: "Save cover asset".to_owned(),
        filters: Vec::new(),
        default_filter: 0,
        default_name: None,
    };

    assert_eq!(dialogs.save_file(&save).unwrap(), Some(output));
    let refusals = [
        dialogs.open_files(&request("Open")).map(|_| ()),
        dialogs.open_file(&request("Open")).map(|_| ()),
        dialogs.pick_folder("Folder").map(|_| ()),
    ];
    for refusal in refusals {
        let error = refusal.expect_err("the smoke test opens only the save dialog");
        assert_eq!(error.source.to_string(), SMOKE_TEST_NO_OTHER_DIALOGS);
    }
}

#[test]
fn driver_passes_after_loading_and_saving() {
    let folder = TempDir::new().unwrap();
    let asset = write_cover_asset(folder.path());
    let output = folder.path().join("saved.asset");
    let mut harness = AppHarness::new();
    harness.dialogs().push_save(output.clone());
    let mut driver = SmokeTestDriver::new(asset.clone(), output.clone(), AssetKind::Cover);

    assert_eq!(drive(&mut harness, &mut driver), SmokeStep::Passed);
    assert_eq!(fs::read(&output).unwrap(), fs::read(&asset).unwrap());
    // Nothing is left unsaved, so closing the window asks nothing.
    let mut app = harness.app();
    assert!(app.request_close());
    assert_eq!(app.modal(), None);
}

#[test]
fn driver_waits_for_running_jobs_after_saving() {
    let folder = TempDir::new().unwrap();
    let asset = write_cover_asset(folder.path());
    let output = folder.path().join("saved.asset");
    // An output file that is there from the start does not pass the run on its own.
    fs::write(&output, "left from an earlier run").unwrap();
    let mut harness = AppHarness::new();
    harness.dialogs().push_save(output.clone());
    let mut driver = SmokeTestDriver::new(asset.clone(), output.clone(), AssetKind::Cover);
    step_until_saved(&mut harness, &mut driver);
    // A load still queued holds the editor while the saved file is already there.
    harness.app().open_paths(vec![asset.clone()]);

    assert_eq!(
        driver.step(&mut harness.app(), STEP_TIME * 2),
        SmokeStep::Waiting
    );
    harness.run_jobs();
    assert_eq!(
        driver.step(&mut harness.app(), STEP_TIME * 3),
        SmokeStep::Passed
    );
    assert_eq!(fs::read(&output).unwrap(), fs::read(&asset).unwrap());
}

#[test]
fn driver_fails_when_a_page_is_unsaved_after_saving() {
    let folder = TempDir::new().unwrap();
    let asset = write_cover_asset(folder.path());
    let output = folder.path().join("saved.asset");
    let mut harness = AppHarness::new();
    harness.dialogs().push_save(output.clone());
    let mut driver = SmokeTestDriver::new(asset, output, AssetKind::Cover);
    step_until_saved(&mut harness, &mut driver);
    harness.app().remove_image(AssetSlot::Cover);

    assert_eq!(
        driver.step(&mut harness.app(), STEP_TIME * 2),
        SmokeStep::Failed("After the save, unsaved changes remain on Cover".to_owned())
    );
}

#[test]
fn driver_fails_when_a_dialog_opens() {
    let folder = TempDir::new().unwrap();
    let asset = folder.path().join("notes.txt");
    fs::write(&asset, "not an asset").unwrap();
    let output = folder.path().join("saved.asset");
    let mut harness = AppHarness::new();
    let mut driver = SmokeTestDriver::new(asset, output.clone(), AssetKind::Cover);

    let SmokeStep::Failed(reason) = drive(&mut harness, &mut driver) else {
        panic!("a dialog fails the smoke test");
    };
    assert!(reason.starts_with("Could not load files"), "{reason}");
    assert!(reason.contains("notes.txt"), "{reason}");
    assert!(!output.exists());
}

#[test]
fn driver_fails_when_startup_left_an_error() {
    let folder = TempDir::new().unwrap();
    let asset = write_cover_asset(folder.path());
    let mut harness = AppHarness::new();
    harness.with_startup_problem(StartupProblem {
        context: "Could not find the local network".to_owned(),
        error: Box::new(io::Error::other("no network adapter")),
    });
    let mut driver =
        SmokeTestDriver::new(asset, folder.path().join("saved.asset"), AssetKind::Cover);

    let step = driver.step(&mut harness.app(), Duration::ZERO);

    assert_eq!(
        step,
        SmokeStep::Failed("Could not find the local network: no network adapter".to_owned())
    );
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn driver_fails_after_the_timeout() {
    let folder = TempDir::new().unwrap();
    let asset = write_cover_asset(folder.path());
    let mut harness = AppHarness::new();
    let mut driver =
        SmokeTestDriver::new(asset, folder.path().join("saved.asset"), AssetKind::Cover);

    assert_eq!(
        driver.step(&mut harness.app(), Duration::ZERO),
        SmokeStep::Waiting
    );
    let late = SMOKE_TEST_TIMEOUT + Duration::from_secs(1);

    assert_eq!(
        driver.step(&mut harness.app(), late),
        SmokeStep::Failed(smoke_timeout_text())
    );
    assert!(
        smoke_timeout_text().contains(&SMOKE_TEST_TIMEOUT.as_secs().to_string()),
        "{}",
        smoke_timeout_text()
    );
}
