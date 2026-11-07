use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use editor_test_support::{
    ImageStatus, PixelBounds, PixelComparison, PixelDifference, ReferenceMode, ReferenceModeError,
    ReferenceStore, read_png, write_opaque_png,
};
use image::{Rgba, RgbaImage};
use tempfile::TempDir;

const IMAGE: &str = "cover-loaded-dark";
const REMEDY: &str = "delete it and run `cargo xtask ui-screenshots update` to recreate it";

/// A store over `references/` and `output/` folders of its own temporary folder.
struct Fixture {
    folder: TempDir,
    store: ReferenceStore,
}

impl Fixture {
    fn new() -> Fixture {
        let folder = tempfile::tempdir().expect("a temporary folder is needed for the store");
        let references = folder.path().join("references");
        let output = folder.path().join("output");
        fs::create_dir(&references).expect("the references folder is created");
        fs::create_dir(&output).expect("the output folder is created");
        Fixture {
            store: ReferenceStore::new(references, output),
            folder,
        }
    }

    fn write_reference(&self, image: &RgbaImage) {
        write_opaque_png(&self.store.reference_path(IMAGE), image)
            .expect("the reference is written");
    }

    fn check(&self, actual: &RgbaImage, mode: ReferenceMode) -> ImageStatus {
        self.store.check(IMAGE, actual, mode)
    }
}

fn plain_image() -> RgbaImage {
    RgbaImage::from_pixel(4, 3, Rgba([10, 20, 30, 255]))
}

fn image_with_one_changed_pixel() -> RgbaImage {
    let mut image = plain_image();
    image.put_pixel(2, 1, Rgba([10, 20, 130, 255]));
    image
}

fn one_changed_pixel() -> PixelComparison {
    PixelComparison::DifferentPixels(PixelDifference {
        count: 1,
        bounds: PixelBounds {
            left: 2,
            top: 1,
            right: 2,
            bottom: 1,
        },
        largest_channel_difference: 100,
    })
}

fn is_emptyfolder(path: &Path) -> bool {
    fs::read_dir(path)
        .expect("the folder is readable")
        .next()
        .is_none()
}

#[test]
fn mode_is_check_by_default() {
    assert_eq!(
        ReferenceMode::from_values(None, None),
        Ok(ReferenceMode::Check)
    );
    assert_eq!(
        ReferenceMode::from_values(Some(""), None),
        Ok(ReferenceMode::Check)
    );
    assert_eq!(
        ReferenceMode::from_values(Some("check"), None),
        Ok(ReferenceMode::Check)
    );
    assert_eq!(
        ReferenceMode::from_values(Some("update"), None),
        Ok(ReferenceMode::Update)
    );
    assert_eq!(
        ReferenceMode::from_values(Some("update"), Some("")),
        Ok(ReferenceMode::Update)
    );
}

#[test]
fn update_mode_is_refused_on_ci() {
    let error = ReferenceMode::from_values(Some("update"), Some("true"))
        .expect_err("update is refused on CI");

    assert_eq!(error, ReferenceModeError::UpdateOnCi);
    assert!(error.to_string().contains("not allowed on CI"));
}

#[test]
fn unknown_mode_is_an_error() {
    let error =
        ReferenceMode::from_values(Some("approve"), None).expect_err("an unknown mode is refused");

    assert_eq!(error, ReferenceModeError::UnknownMode("approve".to_owned()));
    assert_eq!(
        error.to_string(),
        "AURORA_UI_REFERENCES is \"approve\"; use \"check\" or \"update\""
    );
}

#[test]
fn matching_image_writes_only_the_actual() {
    let fixture = Fixture::new();
    fixture.write_reference(&plain_image());

    let status = fixture.check(&plain_image(), ReferenceMode::Check);

    assert_eq!(status, ImageStatus::Matches);
    assert_eq!(
        read_png(&fixture.store.actual_path(IMAGE)).expect("the actual reads back"),
        plain_image()
    );
    assert!(!fixture.store.diff_path(IMAGE).exists());
    assert!(!fixture.store.reference_copy_path(IMAGE).exists());
}

#[test]
fn different_image_fails_with_reference_actual_and_diff() {
    let fixture = Fixture::new();
    fixture.write_reference(&plain_image());
    let reference_bytes =
        fs::read(fixture.store.reference_path(IMAGE)).expect("the reference is readable");

    let status = fixture.check(&image_with_one_changed_pixel(), ReferenceMode::Check);

    assert_eq!(status, ImageStatus::Differs(one_changed_pixel()));
    assert!(status.is_failure());
    assert!(fixture.store.actual_path(IMAGE).exists());
    assert!(fixture.store.reference_copy_path(IMAGE).exists());
    assert!(fixture.store.diff_path(IMAGE).exists());
    assert_eq!(
        fs::read(fixture.store.reference_path(IMAGE)).expect("the reference is readable"),
        reference_bytes
    );
}

#[test]
fn missing_reference_fails_in_check_mode_and_is_never_created() {
    let fixture = Fixture::new();

    let status = fixture.check(&plain_image(), ReferenceMode::Check);

    assert_eq!(status, ImageStatus::Missing);
    assert!(status.is_failure());
    assert!(is_emptyfolder(fixture.store.references()));
    assert!(fixture.store.actual_path(IMAGE).exists());
}

#[test]
fn update_mode_creates_a_missing_reference() {
    let fixture = Fixture::new();

    let status = fixture.check(&plain_image(), ReferenceMode::Update);

    assert_eq!(status, ImageStatus::Created);
    assert!(!status.is_failure());
    assert_eq!(
        read_png(&fixture.store.reference_path(IMAGE)).expect("the reference reads back"),
        plain_image()
    );
}

#[test]
fn update_mode_replaces_a_changed_reference_and_keeps_the_old_for_review() {
    let fixture = Fixture::new();
    fixture.write_reference(&plain_image());

    let status = fixture.check(&image_with_one_changed_pixel(), ReferenceMode::Update);

    assert_eq!(status, ImageStatus::Replaced(one_changed_pixel()));
    assert!(!status.is_failure());
    assert_eq!(
        read_png(&fixture.store.reference_path(IMAGE)).expect("the reference reads back"),
        image_with_one_changed_pixel()
    );
    assert_eq!(
        read_png(&fixture.store.reference_copy_path(IMAGE)).expect("the copy reads back"),
        plain_image()
    );
    assert!(fixture.store.diff_path(IMAGE).exists());
}

#[test]
fn unchanged_reference_keeps_its_bytes_in_update_mode() {
    let fixture = Fixture::new();
    let reference_path = fixture.store.reference_path(IMAGE);
    plain_image()
        .save(&reference_path)
        .expect("the reference is saved with the default PNG settings");
    let bytes_before = fs::read(&reference_path).expect("the reference is readable");

    let status = fixture.check(&plain_image(), ReferenceMode::Update);

    assert_eq!(status, ImageStatus::Matches);
    assert_eq!(
        fs::read(&reference_path).expect("the reference is readable"),
        bytes_before
    );
}

#[test]
fn earlier_outputs_of_an_image_are_removed_before_it_is_checked() {
    let fixture = Fixture::new();
    fixture.write_reference(&plain_image());
    for stale in [
        fixture.store.diff_path(IMAGE),
        fixture.store.reference_copy_path(IMAGE),
    ] {
        fs::create_dir_all(stale.parent().expect("the output has a folder"))
            .expect("the output folder is created");
        fs::write(stale, b"stale").expect("the stale output is written");
    }

    let status = fixture.check(&plain_image(), ReferenceMode::Check);

    assert_eq!(status, ImageStatus::Matches);
    assert!(!fixture.store.diff_path(IMAGE).exists());
    assert!(!fixture.store.reference_copy_path(IMAGE).exists());
}

#[test]
fn translucent_actual_is_an_error() {
    let fixture = Fixture::new();
    let mut actual = plain_image();
    actual.put_pixel(1, 0, Rgba([10, 20, 30, 200]));

    let status = fixture.check(&actual, ReferenceMode::Check);

    let ImageStatus::Error(text) = &status else {
        panic!("a translucent actual is an error, got {status:?}");
    };
    assert!(status.is_failure());
    assert!(text.contains(&fixture.store.actual_path(IMAGE).display().to_string()));
    assert!(text.contains("(1, 0)"));
}

#[test]
fn unreadable_reference_is_an_error_and_is_not_overwritten_in_update_mode() {
    let fixture = Fixture::new();
    let reference_path = fixture.store.reference_path(IMAGE);
    fs::write(&reference_path, b"not a png").expect("the broken reference is written");

    let status = fixture.check(&plain_image(), ReferenceMode::Update);

    let ImageStatus::Error(text) = &status else {
        panic!("an unreadable reference is an error, got {status:?}");
    };
    assert!(text.contains(&reference_path.display().to_string()));
    assert!(text.contains(REMEDY));
    assert_eq!(
        fs::read(&reference_path).expect("the reference is readable"),
        b"not a png"
    );
}

#[test]
fn unreadable_reference_is_an_error_in_check_mode() {
    let fixture = Fixture::new();
    let reference_path = fixture.store.reference_path(IMAGE);
    fs::write(&reference_path, b"not a png").expect("the broken reference is written");

    let status = fixture.check(&plain_image(), ReferenceMode::Check);

    let ImageStatus::Error(text) = &status else {
        panic!("an unreadable reference is an error, got {status:?}");
    };
    assert!(text.contains(&reference_path.display().to_string()));
    assert!(text.contains(REMEDY));
    assert_eq!(
        fs::read(&reference_path).expect("the reference is readable"),
        b"not a png"
    );
}

#[test]
fn translucent_reference_is_an_error_before_any_diff() {
    let fixture = Fixture::new();
    let reference_path = fixture.store.reference_path(IMAGE);
    let mut reference = plain_image();
    reference.put_pixel(0, 2, Rgba([10, 20, 30, 100]));
    reference
        .save(&reference_path)
        .expect("the translucent reference is saved");

    let status = fixture.check(&image_with_one_changed_pixel(), ReferenceMode::Update);

    let ImageStatus::Error(text) = &status else {
        panic!("a translucent reference is an error, got {status:?}");
    };
    assert!(text.contains(&reference_path.display().to_string()));
    assert!(text.contains("(0, 2)"));
    assert!(text.contains(REMEDY));
    assert!(!fixture.store.diff_path(IMAGE).exists());
    assert!(!fixture.store.reference_copy_path(IMAGE).exists());
}

#[test]
fn image_names_that_are_not_kebab_case_are_refused() {
    let fixture = Fixture::new();

    let status = fixture
        .store
        .check("../x", &plain_image(), ReferenceMode::Update);

    let ImageStatus::Error(text) = &status else {
        panic!("a bad name is an error, got {status:?}");
    };
    assert!(text.contains("../x"));
    for result in [
        fixture.store.remove_reference("../x"),
        fixture.store.clear_outputs("../x"),
    ] {
        let error = result.expect_err("a bad name is refused");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("../x"));
    }
    assert!(!fixture.folder.path().join("x.png").exists());
    assert!(!fixture.store.output().join("actual").exists());
}

#[test]
fn reference_names_lists_png_stems() {
    let fixture = Fixture::new();
    for file in ["a.png", "b.png", "notes.txt", "c.png.partial"] {
        fs::write(fixture.store.references().join(file), b"x").expect("the file is written");
    }

    let names = fixture
        .store
        .reference_names()
        .expect("the folder is listed");

    assert_eq!(names, BTreeSet::from(["a".to_owned(), "b".to_owned()]));
}

#[test]
fn reference_names_of_a_missingfolder_is_empty() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the store");
    let store = ReferenceStore::new(
        folder.path().join("references"),
        folder.path().join("output"),
    );

    assert_eq!(
        store.reference_names().expect("a missing folder is empty"),
        BTreeSet::new()
    );
}

#[test]
fn removing_a_reference_deletes_its_file() {
    let fixture = Fixture::new();
    fixture.write_reference(&plain_image());

    fixture
        .store
        .remove_reference(IMAGE)
        .expect("the reference is removed");

    assert!(!fixture.store.reference_path(IMAGE).exists());
}
