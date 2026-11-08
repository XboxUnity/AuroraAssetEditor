use std::any::Any;
use std::thread;

use aurora_asset_editor::settings::ThemeChoice;
use image::RgbaImage;

use super::scene::Scene;

const PANIC_WITHOUT_TEXT: &str = "a panic without a message";

/// Slint's platform, mock clock and focus are per thread, so a fresh thread makes the image
/// independent of every other image and of the order of the catalogue. A panic on that thread
/// becomes the image's failure instead of ending the run.
pub(super) fn capture_on_own_thread(
    image: &str,
    stage: fn(ThemeChoice) -> Scene,
    theme: ThemeChoice,
) -> Result<RgbaImage, String> {
    let capture = thread::Builder::new()
        .name(image.to_owned())
        .spawn(move || stage(theme).capture())
        .map_err(|error| format!("Could not start a thread for {image}: {error}"))?;
    capture.join().map_err(|payload| {
        format!(
            "staging or capture panicked: {}",
            panic_text(payload.as_ref())
        )
    })
}

fn panic_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_owned();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    PANIC_WITHOUT_TEXT.to_owned()
}
