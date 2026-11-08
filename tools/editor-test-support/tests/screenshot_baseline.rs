use std::fs;
use std::path::Path;

use editor_test_support::{
    Baseline, BaselineError, DIFF_PANEL_GAP, ImageChange, MANIFEST_FILE, ManifestError,
    PixelComparison, RunFolder, RunManifest, read_png, write_opaque_png,
};
use image::{Rgba, RgbaImage};
use tempfile::TempDir;

const PLATFORM: &str = "test-platform";
const IMAGE: &str = "x";

fn manifest(platform: &str, filter: Option<&str>) -> RunManifest {
    RunManifest {
        platform: platform.to_owned(),
        label: "local run".to_owned(),
        rendered: "2026-11-08 14:02 UTC".to_owned(),
        filter: filter.map(str::to_owned),
    }
}

fn plain_image() -> RgbaImage {
    RgbaImage::from_pixel(6, 4, Rgba([10, 20, 30, 255]))
}

fn image_with_changed_pixel() -> RgbaImage {
    let mut image = plain_image();
    image.put_pixel(2, 1, Rgba([200, 20, 30, 255]));
    image
}

fn translucent_image() -> RgbaImage {
    let mut image = plain_image();
    image.put_pixel(3, 2, Rgba([10, 20, 30, 254]));
    image
}

/// A baseline folder rendered on `PLATFORM` with `images/x.png` holding `image`, if given.
fn write_baseline(parent: &Path, image: Option<&RgbaImage>) -> Baseline {
    let folder = parent.join("baseline-run");
    manifest(PLATFORM, None)
        .write(&folder)
        .expect("the baseline manifest is written");
    if let Some(image) = image {
        write_opaque_png(&folder.join("images").join("x.png"), image)
            .expect("the baseline image is written");
    }
    Baseline::open(&folder, PLATFORM).expect("the baseline opens")
}

fn run_folder(parent: &Path) -> RunFolder {
    RunFolder::new(parent.join("run"))
}

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("a temporary folder is needed")
}

#[test]
fn manifest_round_trips() {
    for filter in [None, Some("about")] {
        let original = manifest(PLATFORM, filter);
        let read_back =
            RunManifest::from_text(&original.to_text()).expect("the text of a manifest reads back");
        assert_eq!(read_back, original);
    }
    assert_eq!(
        manifest(PLATFORM, None).to_text(),
        "format: 1\nplatform: test-platform\nlabel: local run\nrendered: 2026-11-08 14:02 UTC\n"
    );
}

#[test]
fn manifest_of_an_unknown_format_is_refused() {
    let text = "format: 2\nplatform: test-platform\nlabel: local run\nrendered: now\n";

    assert!(matches!(
        RunManifest::from_text(text),
        Err(ManifestError::UnknownFormat(found)) if found == "2"
    ));
}

#[test]
fn manifest_with_an_unknown_line_is_refused() {
    let text = "format: 1\nplatform: test-platform\ncolour: red\nlabel: local run\nrendered: now\n";

    assert!(matches!(
        RunManifest::from_text(text),
        Err(ManifestError::BadLine { line: 3, .. })
    ));
}

#[test]
fn baseline_of_another_platform_is_refused() {
    let folder = temp_dir();
    manifest("other-platform", None)
        .write(folder.path())
        .expect("the manifest is written");

    let Err(error) = Baseline::open(folder.path(), PLATFORM) else {
        panic!("a baseline of another platform must be refused");
    };

    assert!(matches!(error, BaselineError::OtherPlatform { .. }));
    let text = error.to_string();
    assert!(text.contains("other-platform"), "{text}");
    assert!(text.contains(PLATFORM), "{text}");
}

#[test]
fn filtered_run_is_not_a_baseline() {
    let folder = temp_dir();
    manifest(PLATFORM, Some("about"))
        .write(folder.path())
        .expect("the manifest is written");

    let result = Baseline::open(folder.path(), PLATFORM);

    assert!(matches!(
        result,
        Err(BaselineError::Filtered { filter, .. }) if filter == "about"
    ));
}

#[test]
fn folder_without_a_manifest_is_not_a_baseline() {
    let folder = temp_dir();

    let Err(error) = Baseline::open(folder.path(), PLATFORM) else {
        panic!("a folder without a manifest must be refused");
    };

    assert!(matches!(
        error,
        BaselineError::Manifest {
            error: ManifestError::Missing(_),
            ..
        }
    ));
    let text = error.to_string();
    assert!(text.contains(MANIFEST_FILE), "{text}");
    assert!(
        text.contains(&folder.path().display().to_string()),
        "{text}"
    );
}

#[test]
fn missing_folder_is_not_a_baseline() {
    let folder = temp_dir();
    let missing = folder.path().join("nowhere");

    let result = Baseline::open(&missing, PLATFORM);

    assert!(matches!(result, Err(BaselineError::NotAFolder(path)) if path == missing));
}

#[test]
fn baseline_image_names_are_the_pngs_of_its_images_folder() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), None);
    let images = baseline.folder().join("images");
    fs::create_dir_all(&images).expect("the images folder is created");
    fs::create_dir_all(baseline.folder().join("diff")).expect("the diff folder is created");
    for path in [
        images.join("a.png"),
        images.join("b.png"),
        images.join("notes.txt"),
        baseline.folder().join("diff").join("c.png"),
    ] {
        fs::write(&path, b"x").expect("a file is written");
    }

    let names = baseline.image_names().expect("the names are listed");

    assert_eq!(
        names.into_iter().collect::<Vec<_>>(),
        vec!["a".to_owned(), "b".to_owned()]
    );
}

#[test]
fn unchanged_image_writes_only_the_image() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), Some(&plain_image()));
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &plain_image(), Some(&baseline));

    assert_eq!(change, ImageChange::Unchanged);
    assert_eq!(
        read_png(&run.image_path(IMAGE)).expect("the image is written"),
        plain_image()
    );
    assert!(!run.folder().join("baseline").exists());
    assert!(!run.folder().join("diff").exists());
}

#[test]
fn changed_image_keeps_the_baseline_and_a_diff() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), Some(&plain_image()));
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &image_with_changed_pixel(), Some(&baseline));

    let ImageChange::Changed(PixelComparison::DifferentPixels(difference)) = change else {
        panic!("one changed pixel must give Changed(DifferentPixels), got {change:?}");
    };
    assert_eq!(difference.count, 1);
    assert_eq!(
        fs::read(run.baseline_copy_path(IMAGE)).expect("the baseline copy is written"),
        fs::read(baseline.image_path(IMAGE)).expect("the baseline image is readable")
    );
    let diff = read_png(&run.diff_path(IMAGE)).expect("the diff is written");
    assert_eq!(diff.width(), 3 * 6 + 2 * DIFF_PANEL_GAP);
}

#[test]
fn image_missing_from_the_baseline_is_new() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), None);
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &plain_image(), Some(&baseline));

    assert_eq!(change, ImageChange::New);
    assert!(run.image_path(IMAGE).is_file());
}

#[test]
fn image_without_any_baseline_is_new() {
    let folder = temp_dir();
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &plain_image(), None);

    assert_eq!(change, ImageChange::New);
    assert!(run.image_path(IMAGE).is_file());
}

#[test]
fn unreadable_baseline_image_is_flagged_not_failed() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), None);
    fs::create_dir_all(baseline.folder().join("images")).expect("the images folder is created");
    fs::write(baseline.image_path(IMAGE), b"not a png").expect("the broken image is written");
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &plain_image(), Some(&baseline));

    let ImageChange::BaselineUnreadable(text) = &change else {
        panic!("a baseline that is not a PNG must be flagged, got {change:?}");
    };
    assert!(
        text.contains(&baseline.image_path(IMAGE).display().to_string()),
        "{text}"
    );
    assert!(!change.is_failure());
    assert!(run.image_path(IMAGE).is_file());
}

#[test]
fn translucent_baseline_image_is_flagged_not_failed() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), None);
    fs::create_dir_all(baseline.folder().join("images")).expect("the images folder is created");
    translucent_image()
        .save(baseline.image_path(IMAGE))
        .expect("the translucent image is written");
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &plain_image(), Some(&baseline));

    let ImageChange::BaselineUnreadable(text) = &change else {
        panic!("a translucent baseline image must be flagged, got {change:?}");
    };
    assert!(text.contains("(3, 2)"), "{text}");
    assert!(!change.is_failure());
    assert!(run.image_path(IMAGE).is_file());
}

#[test]
fn translucent_current_image_is_an_error() {
    let folder = temp_dir();
    let run = run_folder(folder.path());

    let change = run.record(IMAGE, &translucent_image(), None);

    let ImageChange::Error(text) = &change else {
        panic!("a translucent current image must be an error, got {change:?}");
    };
    assert!(text.contains("(3, 2)"), "{text}");
    assert!(change.is_failure());
}

#[test]
fn image_names_that_are_not_kebab_case_are_refused() {
    let folder = temp_dir();
    let run = run_folder(folder.path());

    for name in ["../x", "X", ""] {
        let change = run.record(name, &plain_image(), None);

        assert!(
            matches!(change, ImageChange::Error(_)),
            "\"{name}\" gave {change:?}"
        );
    }
    assert!(!folder.path().join("x.png").exists());
    assert!(!run.folder().exists());
}

#[test]
fn removed_image_is_kept_from_the_baseline() {
    let folder = temp_dir();
    let baseline = write_baseline(folder.path(), None);
    fs::create_dir_all(baseline.folder().join("images")).expect("the images folder is created");
    write_opaque_png(&baseline.image_path("gone"), &plain_image())
        .expect("the baseline image is written");
    let run = run_folder(folder.path());

    run.keep_removed("gone", &baseline)
        .expect("the baseline file is copied");

    assert_eq!(
        fs::read(run.baseline_copy_path("gone")).expect("the copy is written"),
        fs::read(baseline.image_path("gone")).expect("the baseline image is readable")
    );
}
