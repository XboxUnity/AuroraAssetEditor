use aurora_asset::AssetSlot;
use aurora_asset_editor::app::Page;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};

use crate::sample_content::{
    SampleAsset, halo_3, load_asset_file, png_file, sample_art, sample_asset_bytes,
};

const COVER_FILE: &str = "GC4D5307E6.asset";
const BACKGROUND_FILE: &str = "BK4D5307E6.asset";
const ICON_BANNER_FILE: &str = "GL4D5307E6.asset";
const REPLACEMENT_COVER_FILE: &str = "new-cover.png";
const REPLACEMENT_COVER_SIZE: (u32, u32) = (600, 400);
const REPLACEMENT_COVER_SEED: u8 = 5;

pub const ASSET_PAGES: ScreenArea = ScreenArea {
    title: "Asset pages",
    screens: &[
        Screen {
            name: "cover-empty",
            description: "The Cover page of a fresh app: placeholder art, no game selected, status \"Ready\".",
            stage: stage_cover_empty,
        },
        Screen {
            name: "cover-loaded",
            description: "The Cover page with a DXT cover loaded for Halo 3: caption \"900 x 600 · DXT\" and the load status.",
            stage: stage_cover_loaded,
        },
        Screen {
            name: "cover-unsupported",
            description: "The Cover page with a tiled cover: \"Unsupported texture format (kept as is)\" in the slot.",
            stage: stage_cover_unsupported,
        },
        Screen {
            name: "cover-drop-hover",
            description: "The drop highlight on the empty cover slot while files are dragged over the window.",
            stage: stage_cover_drop_hover,
        },
        Screen {
            name: "cover-importing",
            description: "A Replace is queued for the loaded cover: the slot's buttons are disabled and the status is unchanged.",
            stage: stage_cover_importing,
        },
        Screen {
            name: "background-empty",
            description: "The Background page with placeholder art.",
            stage: stage_background_empty,
        },
        Screen {
            name: "background-loaded",
            description: "The Background page with an uncompressed background loaded: caption \"1280 x 720 · ARGB\".",
            stage: stage_background_loaded,
        },
        Screen {
            name: "icon-banner-empty",
            description: "The Icon & Banner page with both placeholders.",
            stage: stage_icon_banner_empty,
        },
        Screen {
            name: "icon-banner-loaded",
            description: "The Icon & Banner page with an icon and a banner loaded, each with its buttons.",
            stage: stage_icon_banner_loaded,
        },
        Screen {
            name: "icon-banner-icon-only",
            description: "The Icon & Banner page with the icon filled and the banner empty.",
            stage: stage_icon_banner_icon_only,
        },
    ],
};

fn scene_on_page(theme: ThemeChoice, page: Page) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.show_page(page));
    scene
}

fn loaded_scene(theme: ThemeChoice, page: Page, file_name: &str, asset: SampleAsset) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.show_page(page);
    });
    load_asset_file(&mut scene, file_name, &sample_asset_bytes(asset));
    scene
}

fn stage_cover_empty(theme: ThemeChoice) -> Scene {
    Scene::start(AppHarness::new(), theme)
}

fn stage_cover_loaded(theme: ThemeChoice) -> Scene {
    loaded_scene(theme, Page::Cover, COVER_FILE, SampleAsset::Cover)
}

fn stage_cover_unsupported(theme: ThemeChoice) -> Scene {
    loaded_scene(theme, Page::Cover, COVER_FILE, SampleAsset::TiledCover)
}

fn stage_cover_drop_hover(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.set_drop_hover(true));
    scene
}

fn stage_cover_importing(theme: ThemeChoice) -> Scene {
    let mut scene = loaded_scene(theme, Page::Cover, COVER_FILE, SampleAsset::Cover);
    let replacement = sample_art(
        REPLACEMENT_COVER_SIZE.0,
        REPLACEMENT_COVER_SIZE.1,
        REPLACEMENT_COVER_SEED,
    );
    let path = png_file(&mut scene, REPLACEMENT_COVER_FILE, &replacement);
    scene.harness().dialogs().push_files(vec![path]);
    scene.act(|app| app.replace_image(AssetSlot::Cover));
    scene
}

fn stage_background_empty(theme: ThemeChoice) -> Scene {
    scene_on_page(theme, Page::Background)
}

fn stage_background_loaded(theme: ThemeChoice) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    load_asset_file(
        &mut scene,
        BACKGROUND_FILE,
        &sample_asset_bytes(SampleAsset::Background),
    );
    scene.act(|app| app.show_page(Page::Background));
    scene
}

fn stage_icon_banner_empty(theme: ThemeChoice) -> Scene {
    scene_on_page(theme, Page::IconBanner)
}

fn stage_icon_banner_loaded(theme: ThemeChoice) -> Scene {
    icon_banner_scene(theme, SampleAsset::IconBanner)
}

fn stage_icon_banner_icon_only(theme: ThemeChoice) -> Scene {
    icon_banner_scene(theme, SampleAsset::IconOnly)
}

fn icon_banner_scene(theme: ThemeChoice, asset: SampleAsset) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    load_asset_file(&mut scene, ICON_BANNER_FILE, &sample_asset_bytes(asset));
    scene.act(|app| app.show_page(Page::IconBanner));
    scene
}
