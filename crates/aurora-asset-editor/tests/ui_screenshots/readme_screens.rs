//! The README's screens: staged as their counterparts in the catalogue are, with the user's real
//! art in place of the generated samples.

use aurora_asset::AssetKind;
use aurora_asset_editor::app::Page;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_console::asset_file_name;
use editor_test_support::{AppHarness, Scene, Screen};
use image::imageops;
use test_servers::{RecordedHttp, Route};

use crate::console_screens::{TABLE, start_console};
use crate::online_screens::{
    Cover, THUMBNAIL_SIZE, image_path, json_route, listing_json, listing_path, png_route,
    search_for_title_id, select_result_at, start_online, thumbnail_path,
};
use crate::real_art::{
    README_GAME_TITLE_ID, asset_bytes, console_games, decoded_cover, readme_current_game,
    readme_game, screenshots_bytes, test_assets,
};
use crate::sample_content::{load_asset_file, png_bytes};
use crate::staged_outcomes::{mark_presence, show_games};

/// The one result of the README's online search.
const README_COVER: Cover = Cover {
    label: "NeverDead",
    slug: "neverdead",
    available: true,
};
const FIRST_RESULT: usize = 0;

pub const README_SCREENS: &[Screen] = &[
    Screen {
        name: "cover",
        description: "The Cover page with NeverDead current and its cover loaded.",
        stage: stage_cover,
    },
    Screen {
        name: "screenshots",
        description: "The Screenshots page with NeverDead current and six real screenshots in the strip, the first selected.",
        stage: stage_screenshots,
    },
    Screen {
        name: "console",
        description: "The Console page listing the real console's games with their asset marks, NeverDead selected.",
        stage: stage_console,
    },
    Screen {
        name: "online",
        description: "The Online page after a TitleID search for NeverDead, its one cover selected and shown in full.",
        stage: stage_online,
    },
];

/// The given page with NeverDead current and `bytes` opened as its `kind` file.
fn readme_game_on_page(theme: ThemeChoice, page: Page, kind: AssetKind, bytes: &[u8]) -> Scene {
    let game = readme_current_game();
    let file_name = asset_file_name(kind, &game.ids.title_id);
    let mut scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| {
        app.set_current_game(Some(game));
        app.show_page(page);
    });
    load_asset_file(&mut scene, &file_name, bytes);
    scene
}

fn stage_cover(theme: ThemeChoice) -> Scene {
    let cover = asset_bytes(&readme_game(), AssetKind::Cover);
    readme_game_on_page(theme, Page::Cover, AssetKind::Cover, &cover)
}

fn stage_screenshots(theme: ThemeChoice) -> Scene {
    readme_game_on_page(
        theme,
        Page::Screenshots,
        AssetKind::Screenshots,
        &screenshots_bytes(),
    )
}

fn stage_console(theme: ThemeChoice) -> Scene {
    let games = console_games();
    let scene = start_console(theme);
    scene.act(|app| {
        show_games(app, games.clone());
        for game in &games {
            let presence = test_assets()
                .presence(&game.ids)
                .unwrap_or_else(|error| panic!("{error}"));
            mark_presence(app, &game.ids, Ok(presence));
        }
    });
    scene.click_label_in(&readme_game().title_name, TABLE);
    scene
}

fn stage_online(theme: ThemeChoice) -> Scene {
    let server = RecordedHttp::start(readme_cover_routes());
    let mut scene = start_online(Some(&server), theme);
    scene.keep_alive(server);
    search_for_title_id(&scene, README_GAME_TITLE_ID);
    scene.run_jobs();
    select_result_at(&mut scene, FIRST_RESULT);
    scene
}

/// XboxUnity's listing for NeverDead with its real cover as the one result.
fn readme_cover_routes() -> Vec<Route> {
    let cover = decoded_cover(&readme_game());
    let thumbnail = imageops::thumbnail(&cover, THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1);
    vec![
        json_route(
            &listing_path(README_GAME_TITLE_ID),
            listing_json(README_GAME_TITLE_ID, &[README_COVER]),
        ),
        png_route(&thumbnail_path(&README_COVER), png_bytes(&thumbnail)),
        png_route(&image_path(&README_COVER), png_bytes(&cover)),
    ]
}
