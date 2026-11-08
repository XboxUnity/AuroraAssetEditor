use aurora_asset::ScreenshotNumber;
use aurora_asset_editor::app::Page;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};
use i_slint_backend_testing::ElementHandle;
use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowEvent};

use crate::sample_content::{
    halo_3, load_asset_file, png_file, sample_art, sample_screenshots_bytes,
};

const SCREENSHOTS_FILE: &str = "SS4D5307E6.asset";
const NEW_SCREENSHOT_FILE: &str = "new-screenshot.png";
const NEW_SCREENSHOT_SIZE: (u32, u32) = (1000, 562);
const NEW_SCREENSHOT_SEED: u8 = 7;
const FIRST_THREE: [u8; 3] = [1, 2, 3];
const WITH_GAP: [u8; 3] = [1, 3, 4];
const ALL_TWENTY: [u8; 20] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
];
const EMPTY_SLOT: u8 = 2;
/// Far more than the strip is wide, so the scroll stops at its clamped end.
const SCROLL_TO_END: f32 = -2000.0;

pub const SCREENSHOT_PAGE: ScreenArea = ScreenArea {
    title: "Screenshots page",
    screens: &[
        Screen {
            name: "screenshots-empty",
            description: "The Screenshots page of an app with no screenshots: only the Add tile.",
            stage: stage_empty,
        },
        Screen {
            name: "screenshots-loaded",
            description: "Three screenshots in the strip, the first selected: caption \"1000 x 562 · ARGB · 3 of 20\".",
            stage: stage_loaded,
        },
        Screen {
            name: "screenshots-gaps",
            description: "Screenshots 1, 3 and 4 loaded with the empty position 2 selected.",
            stage: stage_gaps,
        },
        Screen {
            name: "screenshots-full",
            description: "All 20 screenshots loaded: sidebar count 20 and no Add tile.",
            stage: stage_full,
        },
        Screen {
            name: "screenshots-full-minimum",
            description: "All 20 screenshots at the minimum window size, the strip scrolled to its end.",
            stage: stage_full_minimum,
        },
        Screen {
            name: "screenshots-dragging",
            description: "Screenshot 1 dragged over screenshot 3 with the button held: the insertion bar marks the gap after position 3.",
            stage: stage_dragging,
        },
        Screen {
            name: "screenshots-drop-hover",
            description: "The drop highlight on the Screenshots page while files are dragged over the window.",
            stage: stage_drop_hover,
        },
        Screen {
            name: "screenshots-importing",
            description: "An Add is queued: the Add tile and the buttons are disabled.",
            stage: stage_importing,
        },
    ],
};

fn stage_empty(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.show_page(Page::Screenshots));
    scene
}

/// `scene` on the Screenshots page with Halo 3 selected and the given screenshot positions loaded.
fn with_screenshots(mut scene: Scene, positions: &[u8]) -> Scene {
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.show_page(Page::Screenshots);
    });
    load_asset_file(
        &mut scene,
        SCREENSHOTS_FILE,
        &sample_screenshots_bytes(positions),
    );
    scene
}

fn stage_loaded(theme: ThemeChoice) -> Scene {
    with_screenshots(Scene::start(AppHarness::new(), theme), &FIRST_THREE)
}

fn stage_gaps(theme: ThemeChoice) -> Scene {
    let scene = with_screenshots(Scene::start(AppHarness::new(), theme), &WITH_GAP);
    let empty_slot = ScreenshotNumber::new(EMPTY_SLOT).expect("the empty slot exists");
    scene.act(|app| app.select_screenshot(empty_slot));
    scene
}

fn stage_full(theme: ThemeChoice) -> Scene {
    with_screenshots(Scene::start(AppHarness::new(), theme), &ALL_TWENTY)
}

fn stage_full_minimum(theme: ThemeChoice) -> Scene {
    let scene = with_screenshots(
        Scene::start_at_minimum_size(AppHarness::new(), theme),
        &ALL_TWENTY,
    );
    let wheel = centre_of(&scene.element("ScreenshotStrip::wheel"));
    scene.dispatch(WindowEvent::PointerScrolled {
        position: wheel,
        delta_x: 0.0,
        delta_y: SCROLL_TO_END,
    });
    scene
}

fn stage_dragging(theme: ThemeChoice) -> Scene {
    let mut scene = stage_loaded(theme);
    let thumbs = scene.elements("ScreenshotStrip::thumb");
    let start = centre_of(&thumbs[0]);
    let target = centre_of(&thumbs[2]);
    scene.dispatch(WindowEvent::PointerMoved { position: start });
    scene.dispatch(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    scene.dispatch(WindowEvent::PointerMoved { position: target });
    scene.keep_pointer_in_place();
    scene
}

fn stage_drop_hover(theme: ThemeChoice) -> Scene {
    let scene = stage_loaded(theme);
    scene.act(|app| app.set_drop_hover(true));
    scene
}

fn stage_importing(theme: ThemeChoice) -> Scene {
    let mut scene = stage_loaded(theme);
    let screenshot = sample_art(
        NEW_SCREENSHOT_SIZE.0,
        NEW_SCREENSHOT_SIZE.1,
        NEW_SCREENSHOT_SEED,
    );
    let path = png_file(&mut scene, NEW_SCREENSHOT_FILE, &screenshot);
    scene.harness().dialogs().push_files(vec![path]);
    scene.act(|app| app.add_screenshots());
    scene
}

fn centre_of(element: &ElementHandle) -> LogicalPosition {
    let position = element.absolute_position();
    let size = element.size();
    LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    )
}
