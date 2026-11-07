use std::fs;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetFile, AssetKind, AssetSlot, Compression};
use aurora_asset_editor::app::{App, CurrentGame};
use aurora_asset_editor::dialogs::{Modal, ModalAnswer, ModalField};
use aurora_asset_editor::editor::{EditorPages, SlotPreview};
use aurora_asset_editor::files::{asset_save_filter, image_export_filters, save_all_to_folder};
use aurora_asset_editor::platform::{FileFilter, SaveRequest};
use aurora_console::{GameIds, HexId};
use aurora_image::ImageTarget;
use editor_test_support::{AppHarness, DialogRequest, asset_file_with, placeholder};
use image::ImageFormat;
use tempfile::TempDir;

const TITLE_ID: &str = "4D5307E6";

fn halo_3() -> CurrentGame {
    CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse(TITLE_ID).unwrap(),
            database_id: HexId::from_database_value(18),
        },
    }
}

fn shot(number: u8) -> AssetSlot {
    AssetSlot::screenshot(number).unwrap()
}

fn target_of(slot: AssetSlot) -> ImageTarget {
    match slot {
        AssetSlot::Cover => ImageTarget::Cover,
        AssetSlot::Background => ImageTarget::Background,
        AssetSlot::Icon => ImageTarget::Icon,
        AssetSlot::Banner => ImageTarget::Banner,
        _ => ImageTarget::Screenshot,
    }
}

/// An asset file holding the placeholder art in each slot, encoded the way the app encodes it.
fn placeholder_file(slots: &[AssetSlot]) -> AssetFile {
    let entries: Vec<_> = slots
        .iter()
        .map(|slot| (*slot, placeholder(target_of(*slot)), Compression::None))
        .collect();
    asset_file_with(&entries)
}

fn write_file(folder: &Path, name: &str, slots: &[AssetSlot]) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, placeholder_file(slots).to_bytes()).unwrap();
    path
}

fn load(harness: &mut AppHarness, paths: &[&Path]) {
    harness
        .app()
        .open_paths(paths.iter().map(|path| path.to_path_buf()).collect());
    harness.run_jobs();
}

fn last_save_request(harness: &AppHarness) -> SaveRequest {
    match harness.dialogs().requests().last() {
        Some(DialogRequest::SaveFile(request)) => request.clone(),
        other => panic!("expected a save dialog, got {other:?}"),
    }
}

fn message(app: &App) -> (String, String) {
    match app.modal() {
        Some(Modal::Message { title, text }) => (title.clone(), text.clone()),
        other => panic!("expected a message dialog, got {other:?}"),
    }
}

fn title_id_text(app: &App) -> String {
    match app.modal() {
        Some(Modal::TitleId { form, .. }) => form.title_id.clone(),
        other => panic!("expected the TitleID prompt, got {other:?}"),
    }
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn slots_of(kind: AssetKind) -> Vec<AssetSlot> {
    match kind {
        AssetKind::Cover => vec![AssetSlot::Cover],
        AssetKind::Background => vec![AssetSlot::Background],
        AssetKind::IconBanner => vec![AssetSlot::Icon, AssetSlot::Banner],
        AssetKind::Screenshots => vec![shot(1), shot(2)],
    }
}

fn save_all(harness: &mut AppHarness, folder: &Path) {
    harness.dialogs().push_folder(folder.to_path_buf());
    harness.app().save_all_locally();
    harness.app().answer_modal(ModalAnswer::Accept);
}

#[test]
fn per_type_save_offers_default_name_and_asset_filter() {
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo_3()));
    harness.dialogs().push_cancel();

    harness.app().save_kind(AssetKind::Cover);

    let request = last_save_request(&harness);
    assert_eq!(request.title, "Save cover asset");
    assert_eq!(request.default_name.as_deref(), Some("GC4D5307E6.asset"));
    assert_eq!(request.default_filter, 0);
    assert_eq!(request.filters, vec![asset_save_filter()]);
    assert_eq!(
        asset_save_filter(),
        FileFilter {
            name: "Aurora asset file".to_owned(),
            patterns: vec!["*.asset".to_owned()],
        }
    );

    harness.app().set_current_game(None);
    harness.dialogs().push_cancel();
    harness.app().save_kind(AssetKind::Screenshots);
    let request = last_save_request(&harness);
    assert_eq!(request.title, "Save screenshots asset");
    assert_eq!(request.default_name, None);
}

#[test]
fn per_type_save_writes_page_bytes() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let source = write_file(folder.path(), "source.asset", &[AssetSlot::Cover]);
    load(&mut harness, &[&source]);
    let expected = AssetFile::parse(&fs::read(&source).unwrap())
        .unwrap()
        .only(AssetKind::Cover)
        .to_bytes();
    let target = folder.path().join("saved.asset");

    harness.dialogs().push_save(target.clone());
    harness.app().save_kind(AssetKind::Cover);

    assert_eq!(fs::read(target).unwrap(), expected);
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn load_then_save_reproduces_old_app_file() {
    for kind in AssetKind::ALL {
        let source_folder = TempDir::new().unwrap();
        let target_folder = TempDir::new().unwrap();
        let mut harness = AppHarness::new();
        let source = write_file(source_folder.path(), "source.asset", &slots_of(kind));
        load(&mut harness, &[&source]);
        let target = target_folder.path().join("saved.asset");

        harness.dialogs().push_save(target.clone());
        harness.app().save_kind(kind);

        assert_eq!(
            fs::read(target).unwrap(),
            fs::read(source).unwrap(),
            "{kind:?}"
        );
    }
}

#[test]
fn icon_and_banner_save_into_one_gl_file() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let source = write_file(
        folder.path(),
        "GL4D5307E6.asset",
        &[AssetSlot::Icon, AssetSlot::Banner],
    );
    load(&mut harness, &[&source]);
    let target = folder.path().join("out.asset");

    harness.dialogs().push_save(target.clone());
    harness.app().save_kind(AssetKind::IconBanner);

    let original = placeholder_file(&[AssetSlot::Icon, AssetSlot::Banner]);
    let saved = AssetFile::parse(&fs::read(target).unwrap()).unwrap();
    for slot in [AssetSlot::Icon, AssetSlot::Banner] {
        assert_eq!(
            saved.get(slot).unwrap().data,
            original.get(slot).unwrap().data
        );
    }
}

#[test]
fn saving_screenshots_compacts_without_reencoding() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let source = write_file(folder.path(), "SS4D5307E6.asset", &[shot(2), shot(5)]);
    load(&mut harness, &[&source]);
    let target = folder.path().join("out.asset");

    harness.dialogs().push_save(target.clone());
    harness.app().save_kind(AssetKind::Screenshots);

    let original = placeholder_file(&[shot(2), shot(5)]);
    let saved = AssetFile::parse(&fs::read(target).unwrap()).unwrap();
    assert_eq!(
        saved.get(shot(1)).unwrap().data,
        original.get(shot(2)).unwrap().data
    );
    assert_eq!(
        saved.get(shot(2)).unwrap().data,
        original.get(shot(5)).unwrap().data
    );
    assert!(saved.get(shot(3)).is_none());
    assert!(saved.get(shot(5)).is_none());
    let screenshots = &harness.app().pages().screenshots;
    assert!(matches!(
        screenshots.preview(shot(1)),
        SlotPreview::Pixels(_)
    ));
    assert!(matches!(
        screenshots.preview(shot(2)),
        SlotPreview::Pixels(_)
    ));
    assert_eq!(*screenshots.preview(shot(5)), SlotPreview::Empty);
    assert!(screenshots.texture_header(shot(5)).is_none());
}

#[test]
fn save_all_writes_kinds_with_content_or_missing_files() {
    let source_folder = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let cover = write_file(source_folder.path(), "c.asset", &[AssetSlot::Cover]);
    load(&mut harness, &[&cover]);
    fs::write(target.path().join("BK4D5307E6.asset"), b"keep background").unwrap();
    fs::write(target.path().join("SS4D5307E6.asset"), b"keep shots").unwrap();
    harness.app().set_current_game(Some(halo_3()));

    save_all(&mut harness, target.path());

    assert_eq!(harness.app().modal(), None);
    assert_eq!(
        names_in(target.path()),
        [
            "BK4D5307E6.asset",
            "GC4D5307E6.asset",
            "GL4D5307E6.asset",
            "SS4D5307E6.asset"
        ]
    );
    let saved_cover = AssetFile::parse(&fs::read(target.path().join("GC4D5307E6.asset")).unwrap());
    assert!(saved_cover.unwrap().get(AssetSlot::Cover).is_some());
    assert_eq!(
        fs::read(target.path().join("BK4D5307E6.asset")).unwrap(),
        b"keep background"
    );
    assert_eq!(
        fs::read(target.path().join("SS4D5307E6.asset")).unwrap(),
        b"keep shots"
    );
}

#[test]
fn save_all_never_overwrites_with_empty() {
    let target = TempDir::new().unwrap();
    for name in [
        "GC4D5307E6.asset",
        "BK4D5307E6.asset",
        "GL4D5307E6.asset",
        "SS4D5307E6.asset",
    ] {
        fs::write(target.path().join(name), name.as_bytes()).unwrap();
    }
    let mut pages = EditorPages::new();

    let written =
        save_all_to_folder(&mut pages, target.path(), &HexId::parse(TITLE_ID).unwrap()).unwrap();

    assert!(written.is_empty());
    for name in names_in(target.path()) {
        assert_eq!(
            fs::read(target.path().join(&name)).unwrap(),
            name.as_bytes()
        );
    }
}

#[test]
fn save_all_title_id_prompt_rules() {
    let target = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo_3()));

    harness.app().save_all_locally();
    assert_eq!(title_id_text(harness.app()), "4D5307E6");
    match harness.app().modal() {
        Some(Modal::TitleId { title, label, .. }) => {
            assert_eq!(title, "Save all assets");
            assert_eq!(label, "Please specify TitleID:");
        }
        other => panic!("expected the TitleID prompt, got {other:?}"),
    }

    harness
        .app()
        .edit_modal_field(ModalField::TitleId, "4d53-07e6x");
    assert_eq!(title_id_text(harness.app()), "4d5307e6");

    harness.app().edit_modal_field(ModalField::TitleId, "4d5");
    harness.app().answer_modal(ModalAnswer::Accept);
    assert_eq!(title_id_text(harness.app()), "4d5");
    assert!(harness.dialogs().requests().is_empty());

    harness
        .app()
        .edit_modal_field(ModalField::TitleId, "4d5307e6");
    harness.dialogs().push_folder(target.path().to_path_buf());
    harness.app().answer_modal(ModalAnswer::Accept);

    assert_eq!(harness.app().modal(), None);
    assert_eq!(
        harness.dialogs().requests(),
        vec![DialogRequest::PickFolder {
            title: "Select where to save the asset files".to_owned()
        }]
    );
    assert_eq!(
        names_in(target.path()),
        [
            "BK4d5307e6.asset",
            "GC4d5307e6.asset",
            "GL4d5307e6.asset",
            "SS4d5307e6.asset"
        ]
    );
}

#[test]
fn save_all_prompt_cancel_and_empty_prefill() {
    let mut harness = AppHarness::new();

    harness.app().save_all_locally();
    assert_eq!(title_id_text(harness.app()), "");
    harness.app().answer_modal(ModalAnswer::Cancel);

    assert_eq!(harness.app().modal(), None);
    assert!(harness.dialogs().requests().is_empty());
}

#[test]
fn save_failure_is_shown_and_logged() {
    let source_folder = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let cover = write_file(source_folder.path(), "c.asset", &[AssetSlot::Cover]);
    let background = write_file(source_folder.path(), "b.asset", &[AssetSlot::Background]);
    load(&mut harness, &[&cover, &background]);
    harness.app().set_current_game(Some(halo_3()));
    fs::create_dir(target.path().join("GC4D5307E6.asset")).unwrap();

    save_all(&mut harness, target.path());

    let (title, text) = message(harness.app());
    assert_eq!(title, "Could not save");
    assert!(
        text.starts_with("There was an error while saving GC4D5307E6.asset: "),
        "{text}"
    );
    assert!(!text.contains("Already saved"), "{text}");
    assert!(harness.error_log_text().contains("GC4D5307E6.asset"));
    assert!(!target.path().join("BK4D5307E6.asset").exists());

    harness.app().answer_modal(ModalAnswer::Accept);
    fs::remove_dir(target.path().join("GC4D5307E6.asset")).unwrap();
    fs::create_dir(target.path().join("BK4D5307E6.asset")).unwrap();
    save_all(&mut harness, target.path());

    let (title, text) = message(harness.app());
    assert_eq!(title, "Could not save");
    assert!(
        text.starts_with("There was an error while saving BK4D5307E6.asset: "),
        "{text}"
    );
    assert!(
        text.ends_with("\nAlready saved: GC4D5307E6.asset"),
        "{text}"
    );
    assert!(target.path().join("GC4D5307E6.asset").is_file());
}

#[test]
fn per_type_save_failure_is_shown_and_logged() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let source = write_file(folder.path(), "source.asset", &[AssetSlot::Cover]);
    load(&mut harness, &[&source]);
    let target = folder.path().join("GC4D5307E6.asset");
    fs::create_dir(&target).unwrap();

    harness.dialogs().push_save(target);
    harness.app().save_kind(AssetKind::Cover);

    let (title, text) = message(harness.app());
    assert_eq!(title, "Could not save");
    assert!(text.starts_with("There was an error while saving GC4D5307E6.asset: "));
    assert!(harness.error_log_text().contains("Could not save"));
}

fn harness_with_cover_and_screenshot(folder: &Path) -> AppHarness {
    let mut harness = AppHarness::new();
    let cover = write_file(folder, "cover.asset", &[AssetSlot::Cover]);
    let screenshots = write_file(folder, "shots.asset", &[shot(3)]);
    load(&mut harness, &[&cover, &screenshots]);
    harness
}

#[test]
fn export_uses_old_default_names_and_titles() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let icon_banner = write_file(
        folder.path(),
        "all.asset",
        &[AssetSlot::Icon, AssetSlot::Banner],
    );
    load(&mut harness, &[&icon_banner]);
    let cover = write_file(folder.path(), "cover.asset", &[AssetSlot::Cover]);
    let background = write_file(folder.path(), "bg.asset", &[AssetSlot::Background]);
    let shots = write_file(folder.path(), "shots.asset", &[shot(3)]);
    load(&mut harness, &[&cover, &background, &shots]);

    let cases = [
        (
            AssetSlot::Cover,
            "Select where to save the Cover",
            "cover.png",
        ),
        (
            AssetSlot::Background,
            "Select where to save the Background",
            "background.png",
        ),
        (AssetSlot::Icon, "Select where to save the Icon", "icon.png"),
        (
            AssetSlot::Banner,
            "Select where to save the Banner",
            "banner.png",
        ),
        (
            shot(3),
            "Select where to save the Screenshot",
            "screenshot3.png",
        ),
    ];
    for (slot, title, name) in cases {
        harness.dialogs().push_cancel();
        harness.app().export_image(slot);
        let request = last_save_request(&harness);
        assert_eq!(request.title, title);
        assert_eq!(request.default_name.as_deref(), Some(name));
        assert_eq!(request.filters, image_export_filters());
        assert_eq!(request.default_filter, 0);
    }
    let names: Vec<String> = image_export_filters()
        .into_iter()
        .map(|filter| filter.name)
        .collect();
    assert_eq!(names, ["PNG", "JPEG", "BMP", "TIFF", "GIF", "WebP"]);
}

#[test]
fn export_writes_format_by_extension() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_cover_and_screenshot(folder.path());
    let cases = [
        ("a.png", ImageFormat::Png),
        ("a.JPG", ImageFormat::Jpeg),
        ("a.bmp", ImageFormat::Bmp),
        ("a.tiff", ImageFormat::Tiff),
        ("a.gif", ImageFormat::Gif),
        ("a.webp", ImageFormat::WebP),
    ];
    for (name, format) in cases {
        let target = folder.path().join(name);
        harness.dialogs().push_save(target.clone());
        harness.app().export_image(AssetSlot::Cover);
        let bytes = fs::read(&target).unwrap();
        assert_eq!(image::guess_format(&bytes).unwrap(), format, "{name}");
    }
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn export_without_extension_writes_png() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_cover_and_screenshot(folder.path());
    let target = folder.path().join("cover");

    harness.dialogs().push_save(target.clone());
    harness.app().export_image(AssetSlot::Cover);

    let bytes = fs::read(target).unwrap();
    assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Png);
}

#[test]
fn export_of_loaded_entry_uses_decoded_pixels() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_cover_and_screenshot(folder.path());
    let target = folder.path().join("cover.png");
    let SlotPreview::Pixels(shown) = harness
        .app()
        .pages()
        .cover
        .preview(AssetSlot::Cover)
        .clone()
    else {
        panic!("the loaded cover shows pixels");
    };

    harness.dialogs().push_save(target.clone());
    harness.app().export_image(AssetSlot::Cover);

    let exported =
        image::load_from_memory_with_format(&fs::read(target).unwrap(), ImageFormat::Png)
            .unwrap()
            .into_rgba8();
    assert_eq!(exported, shown);
}

#[test]
fn export_of_empty_slot_does_nothing() {
    let mut harness = AppHarness::new();

    harness.app().export_image(AssetSlot::Cover);

    assert!(harness.dialogs().requests().is_empty());
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn export_failure_is_shown_and_logged() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_cover_and_screenshot(folder.path());
    let target = folder.path().join("cover.png");
    fs::create_dir(&target).unwrap();

    harness.dialogs().push_save(target);
    harness.app().export_image(AssetSlot::Cover);

    let (title, text) = message(harness.app());
    assert_eq!(title, "Could not save");
    assert!(text.starts_with("There was an error while saving cover.png: "));
    assert!(harness.error_log_text().contains("Could not save"));
}

#[test]
fn dialog_failure_on_save_is_shown() {
    let mut harness = AppHarness::new();
    harness.dialogs().push_failure("no portal");

    harness.app().save_kind(AssetKind::Cover);

    let (title, text) = message(harness.app());
    assert_eq!(title, "Could not open the file dialog");
    assert!(text.contains("no portal"), "{text}");
}

#[test]
fn non_ascii_paths_save() {
    let source_folder = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let folder = target.path().join("ゲーム");
    fs::create_dir(&folder).unwrap();
    let mut harness = AppHarness::new();
    let source = write_file(source_folder.path(), "c.asset", &[AssetSlot::Cover]);
    load(&mut harness, &[&source]);
    harness.app().set_current_game(Some(halo_3()));

    harness.dialogs().push_save(folder.join("カバー.asset"));
    harness.app().save_kind(AssetKind::Cover);
    save_all(&mut harness, &folder);

    assert_eq!(harness.app().modal(), None);
    assert!(folder.join("カバー.asset").is_file());
    assert!(folder.join("GC4D5307E6.asset").is_file());
}
