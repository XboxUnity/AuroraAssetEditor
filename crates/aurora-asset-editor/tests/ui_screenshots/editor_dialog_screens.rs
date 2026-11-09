use std::path::PathBuf;

use aurora_asset::{AssetFile, AssetSlot, ScreenshotNumber};
use aurora_asset_editor::app::Page;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};

use crate::sample_content::{
    SampleAsset, console_harness, fsd_with_screenshots, halo_3, load_asset_file, png_file,
    sample_art, sample_asset_bytes, sample_games, sample_screenshots_bytes, text_file,
    undecodable_entry_asset,
};
use crate::staged_outcomes::show_games;

const MISSING_FILE: &str = "missing-file.asset";
const NOTES_FILE: &str = "notes.asset";
const LONG_PROBLEM_FILES: usize = 40;
const EMPTY_ASSET_FILE: &str = "GC00000000.asset";
const UNREAD_IMAGES_FILE: &str = "SS4D5307E6.asset";
const COVER_FILE: &str = "GC4D5307E6.asset";
const SCREENSHOTS_FILE: &str = "SS4D5307E6.asset";
const FSD_FILE: &str = "game.assets";
const ART_FILE: &str = "art.png";
const ART_SIZE: (u32, u32) = (64, 64);
const ART_SEED: u8 = 5;
const FIRST_THREE: [u8; 3] = [1, 2, 3];
const NINETEEN: [u8; 19] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];
const ALL_TWENTY: [u8; 20] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
];
const FSD_SCREENSHOTS: usize = 3;
const TYPED_TITLE_ID: &str = "4D5";
const DIALOG_SERVICE_FAILURE: &str = "the file dialog service is not running";
const PICKER_FILTER: &str = "GamePicker::filter-field";
/// Part of the titles Gears of War 2 and Perfect Dark.
const TITLE_FILTER: &str = "ar";

pub const EDITOR_DIALOGS: ScreenArea = ScreenArea {
    title: "Editor dialogs",
    screens: &[
        Screen {
            name: "dialog-load-problems",
            description: "\"Could not load files\" listing a file that does not exist and a text file named notes.asset.",
            stage: stage_load_problems,
        },
        Screen {
            name: "dialog-load-problems-long",
            description: "\"Could not load files\" for 40 missing files: the message is capped in height and scrolls.",
            stage: stage_load_problems_long,
        },
        Screen {
            name: "dialog-no-assets",
            description: "\"No assets\": GC00000000.asset doesn't contain any assets.",
            stage: stage_no_assets,
        },
        Screen {
            name: "dialog-unread-images",
            description: "\"Some images could not be read\" after loading a file with one undecodable entry.",
            stage: stage_unread_images,
        },
        Screen {
            name: "dialog-not-enough-space",
            description: "\"Not enough space\" after loading an FSD file with three screenshots into 19 existing ones.",
            stage: stage_not_enough_space,
        },
        Screen {
            name: "dialog-no-space",
            description: "\"No space left\" after adding a screenshot to a full set of 20.",
            stage: stage_no_space,
        },
        Screen {
            name: "dialog-image-failed",
            description: "\"Could not use image\" over the kept cover, after replacing it with a text file named art.png.",
            stage: stage_image_failed,
        },
        Screen {
            name: "dialog-icon-or-banner",
            description: "\"Icon or banner?\" with Icon, Banner and Cancel, after dropping an image on the Icon & Banner page.",
            stage: stage_icon_or_banner,
        },
        Screen {
            name: "dialog-replace-screenshot",
            description: "\"Replace screenshot?\" with Replace, Add to free slots and Cancel, after dropping an image on three loaded screenshots.",
            stage: stage_replace_screenshot,
        },
        Screen {
            name: "dialog-save-all",
            description: "\"Save all assets\" with the current game's TitleID, 4D5307E6, prefilled; no console is set up, so there is no game picker.",
            stage: stage_save_all,
        },
        Screen {
            name: "dialog-save-all-invalid",
            description: "\"Save all assets\" with the incomplete TitleID 4D5 typed: OK is disabled.",
            stage: stage_save_all_invalid,
        },
        Screen {
            name: "dialog-save-all-loading",
            description: "\"Save all assets\" with the console set up and its games not listed yet: the prefilled TitleID 4D5307E6 above \"Refreshing the game list…\" in the picker.",
            stage: stage_save_all_loading,
        },
        Screen {
            name: "dialog-save-all-filtered",
            description: "\"Save all assets\" over the listed sample games with ar typed in the picker: Gears of War 2 and Perfect Dark, the first highlighted; the TitleID box still holds 4D5307E6.",
            stage: stage_save_all_filtered,
        },
        Screen {
            name: "dialog-discard-changes",
            description: "\"Unsaved changes\": \"Discard unsaved changes to Cover and Screenshots?\" with Discard in the danger colour and Cancel focused, after New asset set over a replaced cover and a removed screenshot.",
            stage: stage_discard_changes,
        },
        Screen {
            name: "dialog-file-dialog-failed",
            description: "\"Could not open the file dialog\" after the platform's file dialog fails to open.",
            stage: stage_file_dialog_failed,
        },
    ],
};

fn start(theme: ThemeChoice) -> Scene {
    Scene::start(AppHarness::new(), theme)
}

/// A scene with Halo 3 selected.
fn start_with_game(theme: ThemeChoice) -> Scene {
    let scene = start(theme);
    scene.act(|app| app.set_current_game(Some(halo_3())));
    scene
}

fn open_paths(scene: &mut Scene, paths: Vec<PathBuf>) {
    scene.act(|app| app.open_paths(paths));
    scene.run_jobs();
}

fn art_file(scene: &mut Scene) -> PathBuf {
    let art = sample_art(ART_SIZE.0, ART_SIZE.1, ART_SEED);
    png_file(scene, ART_FILE, &art)
}

fn with_cover(mut scene: Scene) -> Scene {
    load_asset_file(
        &mut scene,
        COVER_FILE,
        &sample_asset_bytes(SampleAsset::Cover),
    );
    scene
}

fn with_screenshots(mut scene: Scene, positions: &[u8]) -> Scene {
    load_asset_file(
        &mut scene,
        SCREENSHOTS_FILE,
        &sample_screenshots_bytes(positions),
    );
    scene
}

fn stage_load_problems(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    let notes = text_file(&mut scene, NOTES_FILE);
    open_paths(&mut scene, vec![PathBuf::from(MISSING_FILE), notes]);
    scene
}

fn stage_load_problems_long(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    let missing = (0..LONG_PROBLEM_FILES)
        .map(|number| PathBuf::from(format!("missing-{number}.asset")))
        .collect();
    open_paths(&mut scene, missing);
    scene
}

fn stage_no_assets(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    load_asset_file(&mut scene, EMPTY_ASSET_FILE, &AssetFile::new().to_bytes());
    scene
}

fn stage_unread_images(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    load_asset_file(&mut scene, UNREAD_IMAGES_FILE, &undecodable_entry_asset());
    scene
}

fn stage_not_enough_space(theme: ThemeChoice) -> Scene {
    let mut scene = with_screenshots(start(theme), &NINETEEN);
    load_asset_file(&mut scene, FSD_FILE, &fsd_with_screenshots(FSD_SCREENSHOTS));
    scene
}

fn stage_no_space(theme: ThemeChoice) -> Scene {
    let mut scene = with_screenshots(start(theme), &ALL_TWENTY);
    let art = art_file(&mut scene);
    scene.harness().dialogs().push_files(vec![art]);
    scene.act(|app| app.add_screenshots());
    scene.run_jobs();
    scene
}

fn stage_image_failed(theme: ThemeChoice) -> Scene {
    let mut scene = with_cover(start(theme));
    let not_an_image = text_file(&mut scene, ART_FILE);
    scene.harness().dialogs().push_files(vec![not_an_image]);
    scene.act(|app| app.replace_image(AssetSlot::Cover));
    scene.run_jobs();
    scene
}

fn stage_icon_or_banner(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    scene.act(|app| app.show_page(Page::IconBanner));
    let art = art_file(&mut scene);
    scene.act(|app| app.files_dropped(vec![art]));
    scene
}

fn stage_replace_screenshot(theme: ThemeChoice) -> Scene {
    let mut scene = with_screenshots(start_with_game(theme), &FIRST_THREE);
    let art = art_file(&mut scene);
    scene.act(|app| app.files_dropped(vec![art]));
    scene
}

fn stage_save_all(theme: ThemeChoice) -> Scene {
    let scene = start_with_game(theme);
    scene.act(|app| app.save_all_locally());
    scene
}

fn stage_save_all_invalid(theme: ThemeChoice) -> Scene {
    let scene = stage_save_all(theme);
    scene.type_text(TYPED_TITLE_ID);
    scene
}

/// Save all with Halo 3 selected and the console set up, its games not listed yet.
fn stage_save_all_loading(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.save_all_locally();
    });
    scene
}

fn stage_save_all_filtered(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| {
        show_games(app, sample_games());
        app.set_current_game(Some(halo_3()));
        app.save_all_locally();
    });
    scene.click(PICKER_FILTER);
    scene.type_text(TITLE_FILTER);
    scene
}

fn stage_discard_changes(theme: ThemeChoice) -> Scene {
    let mut scene = with_screenshots(with_cover(start(theme)), &FIRST_THREE);
    let art = art_file(&mut scene);
    scene.harness().dialogs().push_files(vec![art]);
    scene.act(|app| app.replace_image(AssetSlot::Cover));
    scene.run_jobs();
    let second = ScreenshotNumber::new(2).expect("position 2 exists");
    scene.act(|app| {
        app.remove_image(AssetSlot::Screenshot(second));
        app.show_page(Page::Cover);
        app.new_asset_set();
    });
    scene
}

fn stage_file_dialog_failed(theme: ThemeChoice) -> Scene {
    let mut scene = start(theme);
    scene
        .harness()
        .dialogs()
        .push_failure(DIALOG_SERVICE_FAILURE);
    scene.act(|app| app.replace_image(AssetSlot::Cover));
    scene
}
