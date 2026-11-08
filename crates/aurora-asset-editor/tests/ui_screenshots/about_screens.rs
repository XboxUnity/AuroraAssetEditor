use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};
use slint::platform::WindowEvent;

use crate::screenshot_page_screens::centre_of;

const CLIPBOARD_FAILURE: &str = "clipboard is gone";
/// Far more than the card's body is taller than the window at the minimum size, so the scroll stops at its end.
const SCROLL_TO_END: f32 = -2000.0;

// The AboutSlint widget prints the version of Slint it comes with, so a Slint upgrade changes every screen here
// that shows it (all but about-copy-failed) and they must be approved again. The notices screens show only the top
// of notices/handwritten.txt, so editing its head changes them; the generated third-party list further down does not.
pub const ABOUT: ScreenArea = ScreenArea {
    title: "About",
    screens: &[
        Screen {
            name: "about",
            description: "The About dialog with the build lines 2.0.7 (a1b2c3d), the credits and \"Made with Slint\", the notices collapsed. The Slint widget prints its own version, which changes with every Slint upgrade.",
            stage: stage_about,
        },
        Screen {
            name: "about-copied",
            description: "The About dialog after \"Copy build info\": the button reads \"Copied\". The Slint widget prints its own version, which changes with every Slint upgrade.",
            stage: stage_copied,
        },
        Screen {
            name: "about-copy-failed",
            description: "\"Could not copy the build info\" after \"Copy build info\" in an app whose clipboard fails: the About dialog closes to show the error, whose text gives the reason \"Could not use the clipboard: clipboard is gone\".",
            stage: stage_copy_failed,
        },
        Screen {
            name: "about-notices",
            description: "The About dialog with the third-party notices expanded, showing the top of the list: the whole card, \"Made with Slint\" included, fits the window. The box shows the head of notices/handwritten.txt. The Slint widget prints its own version, which changes with every Slint upgrade.",
            stage: stage_notices,
        },
        Screen {
            name: "about-notices-minimum",
            description: "The About dialog with the notices expanded at the minimum window size, its body scrolled to the end: \"Made with Slint\" shows below the notices box while the title and the OK row stay in view. The box shows the head of notices/handwritten.txt. The Slint widget prints its own version, which changes with every Slint upgrade.",
            stage: stage_notices_minimum,
        },
    ],
};

fn stage_about(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.show_about());
    scene
}

fn stage_copied(theme: ThemeChoice) -> Scene {
    let scene = stage_about(theme);
    scene.act(|app| app.copy_build_info());
    scene
}

fn stage_copy_failed(theme: ThemeChoice) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    scene.harness().clipboard().fail_with(CLIPBOARD_FAILURE);
    scene.act(|app| {
        app.show_about();
        app.copy_build_info();
    });
    scene
}

fn stage_notices(theme: ThemeChoice) -> Scene {
    let scene = stage_about(theme);
    scene.act(|app| app.toggle_about_notices());
    scene
}

/// The wheel turns over "Copy build info", outside the notices list, so the card's body scrolls and not the list.
fn stage_notices_minimum(theme: ThemeChoice) -> Scene {
    let scene = Scene::start_at_minimum_size(AppHarness::new(), theme);
    scene.act(|app| {
        app.show_about();
        app.toggle_about_notices();
    });
    let copy_button = centre_of(&scene.element("AboutDialog::copy-button"));
    scene.dispatch(WindowEvent::PointerScrolled {
        position: copy_button,
        delta_x: 0.0,
        delta_y: SCROLL_TO_END,
    });
    scene
}
