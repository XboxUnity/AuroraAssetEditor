use aurora_ui::{MainWindow, ShellModel};
use editor_test_support::HeadlessUi;
use image::{Rgba, RgbaImage};
use slint::ComponentHandle;

const DARK_WINDOW_BACKGROUND: Rgba<u8> = Rgba([0x1b, 0x1d, 0x22, 0xff]);
const LIGHT_WINDOW_BACKGROUND: Rgba<u8> = Rgba([0xf5, 0xf6, 0xf8, 0xff]);
const MIN_WINDOW_BACKGROUND_SHARE: f64 = 0.05;

fn share_of_pixels(image: &RgbaImage, color: Rgba<u8>) -> f64 {
    let matching = image.pixels().filter(|pixel| **pixel == color).count();
    matching as f64 / image.pixels().len() as f64
}

fn render_main_window(dark: bool) -> RgbaImage {
    let ui = HeadlessUi::new();
    let window = MainWindow::new().expect("the main window builds on the testing backend");
    window.global::<ShellModel>().set_dark(dark);
    ui.show(window.window());
    ui.snapshot(window.window())
}

#[test]
fn main_window_renders_headless_in_both_themes() {
    let (width, height) = HeadlessUi::WINDOW_SIZE;

    let dark = render_main_window(true);
    assert_eq!(dark.dimensions(), (width, height));
    assert!(share_of_pixels(&dark, DARK_WINDOW_BACKGROUND) >= MIN_WINDOW_BACKGROUND_SHARE);

    let light = render_main_window(false);
    assert_eq!(light.dimensions(), (width, height));
    assert!(share_of_pixels(&light, LIGHT_WINDOW_BACKGROUND) >= MIN_WINDOW_BACKGROUND_SHARE);
}

slint::slint! {
    export component TypingWindow inherits Window {
        in-out property <string> typed <=> input.text;
        forward-focus: input;
        input := TextInput { }
    }
}

#[test]
fn type_text_reaches_the_focused_text_input() {
    let ui = HeadlessUi::new();
    let window = TypingWindow::new().expect("the typing window builds on the testing backend");
    ui.show(window.window());

    ui.type_text(window.window(), "4D5307E6 Halo");

    assert_eq!(window.get_typed(), "4D5307E6 Halo");
}
