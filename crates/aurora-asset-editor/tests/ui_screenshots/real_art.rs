//! The user's real Aurora assets, for the README screens only. Every function panics with a message
//! naming the file or row it could not get, so a README image fails with its reason.

use std::sync::OnceLock;

use aurora_asset::{AssetKind, AssetSlot, decode};
use aurora_asset_editor::app::CurrentGame;
use aurora_console::{Game, asset_file_name};
use editor_test_support::TestAssets;
use image::RgbaImage;

/// NeverDead: a genuine Xbox 360 box whose bold title stays legible on the cover page.
pub const README_GAME_TITLE_ID: &str = "4B4E081F";

/// The folder named by `AURORA_TEST_ASSETS`.
pub fn test_assets() -> &'static TestAssets {
    static ASSETS: OnceLock<TestAssets> = OnceLock::new();
    ASSETS.get_or_init(|| TestAssets::from_environment().unwrap_or_else(|error| panic!("{error}")))
}

fn game_with_title_id(title_id: &str) -> Game {
    console_games()
        .into_iter()
        .find(|game| game.ids.title_id.as_str() == title_id)
        .unwrap_or_else(|| {
            panic!(
                "the content.db in {} has no game with the TitleID {title_id}",
                test_assets().folder().display()
            )
        })
}

pub fn readme_game() -> Game {
    game_with_title_id(README_GAME_TITLE_ID)
}

pub fn readme_current_game() -> CurrentGame {
    let game = readme_game();
    CurrentGame {
        title: game.title_name,
        ids: game.ids,
    }
}

/// Every game of the folder's `content.db`, in its order.
pub fn console_games() -> Vec<Game> {
    test_assets()
        .games()
        .unwrap_or_else(|error| panic!("{error}"))
}

pub fn asset_bytes(game: &Game, kind: AssetKind) -> Vec<u8> {
    test_assets()
        .asset_bytes(&game.ids, kind)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// The game's cover as the app shows it.
pub fn decoded_cover(game: &Game) -> RgbaImage {
    let texture = test_assets()
        .texture(&game.ids, AssetKind::Cover, AssetSlot::Cover)
        .unwrap_or_else(|error| panic!("{error}"));
    decode(&texture).unwrap_or_else(|error| {
        panic!(
            "the cover in {} of {} could not be decoded: {error}",
            asset_file_name(AssetKind::Cover, &game.ids.title_id),
            test_assets().folder().display()
        )
    })
}
