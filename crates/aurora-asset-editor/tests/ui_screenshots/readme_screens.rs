//! The README's screens: staged as their counterparts in the catalogue are, with the user's real
//! art in place of the generated samples. NeverDead is the current game on every screen, with its
//! full set loaded.

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
    readme_game, test_assets,
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
        description: "The Cover page with NeverDead's full set loaded.",
        stage: stage_cover,
    },
    Screen {
        name: "screenshots",
        description: "The Screenshots page with NeverDead's full set loaded: its two screenshots in the strip, the first selected.",
        stage: stage_screenshots,
    },
    Screen {
        name: "console",
        description: "The Console page listing the real console's games with their asset marks, NeverDead selected and its full set loaded.",
        stage: stage_console,
    },
    Screen {
        name: "online",
        description: "The Online page after a TitleID search for NeverDead, its one cover selected and shown in full, NeverDead's full set loaded.",
        stage: stage_online,
    },
];

/// Makes NeverDead the current game and opens every one of its asset files, `last` last, so the
/// status line names that file.
fn load_readme_game(scene: &mut Scene, last: AssetKind) {
    let game = readme_game();
    scene.act(|app| app.set_current_game(Some(readme_current_game())));
    let others = AssetKind::ALL.into_iter().filter(|kind| *kind != last);
    for kind in others.chain([last]) {
        let file_name = asset_file_name(kind, &game.ids.title_id);
        load_asset_file(scene, &file_name, &asset_bytes(&game, kind));
    }
}

/// NeverDead's full set on the page that shows `kind`.
fn readme_game_on_page(theme: ThemeChoice, page: Page, kind: AssetKind) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    load_readme_game(&mut scene, kind);
    scene.act(|app| app.show_page(page));
    scene
}

fn stage_cover(theme: ThemeChoice) -> Scene {
    readme_game_on_page(theme, Page::Cover, AssetKind::Cover)
}

fn stage_screenshots(theme: ThemeChoice) -> Scene {
    readme_game_on_page(theme, Page::Screenshots, AssetKind::Screenshots)
}

fn stage_console(theme: ThemeChoice) -> Scene {
    let games = console_games();
    let mut scene = start_console(theme);
    load_readme_game(&mut scene, AssetKind::Cover);
    scene.act(|app| {
        app.show_page(Page::Console);
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
    load_readme_game(&mut scene, AssetKind::Cover);
    scene.act(|app| app.show_page(Page::Online));
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
