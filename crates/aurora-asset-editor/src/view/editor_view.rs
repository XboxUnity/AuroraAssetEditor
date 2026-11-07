use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::{AssetSlot, SCREENSHOT_COUNT, ScreenshotNumber, TextureHeader};
use aurora_image::ImageTarget;
use aurora_ui::{EditorModel, MainWindow, ScreenshotTile, SlotView};
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::app::App;
use crate::editor::{AssetDocument, SlotPreview, texture_caption};

use super::ImageCache;
use super::shell_view::{Refresh, action_runner};

/// Connects the asset pages' buttons, the screenshot strip and drag-reorder to `app`.
pub(super) fn connect_editor(window: &MainWindow, app: &Rc<RefCell<App>>, refresh: &Refresh) {
    let editor = window.global::<EditorModel>();

    let act = action_runner(app, refresh);
    editor.on_replace(move |index| {
        if let Some(slot) = slot_of(index) {
            act(&mut |app| app.replace_image(slot));
        }
    });

    let act = action_runner(app, refresh);
    editor.on_export(move |index| {
        if let Some(slot) = slot_of(index) {
            act(&mut |app| app.export_image(slot));
        }
    });

    let act = action_runner(app, refresh);
    editor.on_remove(move |index| {
        if let Some(slot) = slot_of(index) {
            act(&mut |app| app.remove_image(slot));
        }
    });

    let act = action_runner(app, refresh);
    editor.on_add_screenshots(move || act(&mut |app| app.add_screenshots()));

    let act = action_runner(app, refresh);
    editor.on_select_screenshot(move |number| {
        if let Some(number) = screenshot_number_of(number) {
            act(&mut |app| app.select_screenshot(number));
        }
    });

    let act = action_runner(app, refresh);
    editor.on_move_screenshot(move |from, to| {
        if let (Some(from), Some(to)) = (screenshot_number_of(from), screenshot_number_of(to)) {
            act(&mut |app| app.move_screenshot(from, to));
        }
    });
}

pub(super) fn render_editor(window: &MainWindow, app: &App, images: &mut ImageCache) {
    let editor = window.global::<EditorModel>();
    let pages = app.pages();
    editor.set_cover(slot_view(&pages.cover, AssetSlot::Cover, images));
    editor.set_background(slot_view(&pages.background, AssetSlot::Background, images));
    editor.set_icon(slot_view(&pages.icon_banner, AssetSlot::Icon, images));
    editor.set_banner(slot_view(&pages.icon_banner, AssetSlot::Banner, images));

    let screenshots = &pages.screenshots;
    let selected = app.selected_screenshot();
    let mut screenshot = slot_view(screenshots, AssetSlot::Screenshot(selected), images);
    screenshot.caption = format!(
        "{} · {} of {SCREENSHOT_COUNT}",
        screenshot.caption,
        screenshots.screenshot_count()
    )
    .into();
    editor.set_screenshot(screenshot);
    editor.set_screenshot_tiles(screenshot_tiles(screenshots, selected, images));
    editor.set_drop_hover(app.drop_hover());
}

fn slot_view(document: &AssetDocument, slot: AssetSlot, images: &mut ImageCache) -> SlotView {
    let preview = document.preview(slot);
    let header = document.texture_header(slot);
    SlotView {
        slot_index: slot_index(slot),
        title: slot_title(slot).into(),
        caption: slot_caption(slot, header).into(),
        image: slot_image(document, slot, images),
        has_image: matches!(preview, SlotPreview::Pixels(_)),
        unsupported: matches!(preview, SlotPreview::Unsupported(_)),
        can_export: matches!(preview, SlotPreview::Pixels(_)),
        can_remove: header.is_some(),
    }
}

/// Positions 1 to the last filled one, then the "+" tile for the position after it while there is one.
fn screenshot_tiles(
    screenshots: &AssetDocument,
    selected: ScreenshotNumber,
    images: &mut ImageCache,
) -> ModelRc<ScreenshotTile> {
    let is_filled = |number: ScreenshotNumber| {
        screenshots
            .texture_header(AssetSlot::Screenshot(number))
            .is_some()
    };
    let shown_count = ScreenshotNumber::ALL
        .iter()
        .rposition(|number| is_filled(*number))
        .map_or(0, |last| last + 1);
    let mut tiles: Vec<ScreenshotTile> = ScreenshotNumber::ALL[..shown_count]
        .iter()
        .map(|number| {
            let slot = AssetSlot::Screenshot(*number);
            ScreenshotTile {
                number: i32::from(number.get()),
                image: slot_image(screenshots, slot, images),
                filled: is_filled(*number),
                selected: *number == selected,
                is_next_free: false,
            }
        })
        .collect();
    if let Some(next_free) = ScreenshotNumber::ALL.get(shown_count) {
        tiles.push(ScreenshotTile {
            number: i32::from(next_free.get()),
            image: Image::default(),
            filled: false,
            selected: *next_free == selected,
            is_next_free: true,
        });
    }
    ModelRc::new(VecModel::from(tiles))
}

fn slot_image(document: &AssetDocument, slot: AssetSlot, images: &mut ImageCache) -> Image {
    images.image(slot, document.revision(slot), || {
        match document.preview(slot) {
            SlotPreview::Pixels(pixels) => {
                let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    pixels.as_raw(),
                    pixels.width(),
                    pixels.height(),
                );
                Image::from_rgba8(buffer)
            }
            SlotPreview::Empty | SlotPreview::Unsupported(_) => Image::default(),
        }
    })
}

fn slot_title(slot: AssetSlot) -> String {
    match slot {
        AssetSlot::Cover => "Cover".to_owned(),
        AssetSlot::Background => "Background".to_owned(),
        AssetSlot::Icon => "Icon".to_owned(),
        AssetSlot::Banner => "Banner".to_owned(),
        AssetSlot::Slot => String::new(),
        AssetSlot::Screenshot(number) => format!("Screenshot {}", number.get()),
    }
}

/// "{w} x {h} · {format}" of the stored texture, or the target size and "empty".
fn slot_caption(slot: AssetSlot, header: Option<&TextureHeader>) -> String {
    if let Some(header) = header {
        return texture_caption(header);
    }
    let (width, height) = ImageTarget::for_slot(slot).map_or((0, 0), ImageTarget::size);
    format!("{width} x {height} · empty")
}

fn slot_index(slot: AssetSlot) -> i32 {
    i32::try_from(slot.index()).unwrap_or(i32::MAX)
}

fn slot_of(index: i32) -> Option<AssetSlot> {
    let slot = usize::try_from(index).ok().and_then(AssetSlot::from_index);
    if slot.is_none() {
        log::error!("The window named entry {index}, which an asset file does not have");
    }
    slot
}

fn screenshot_number_of(number: i32) -> Option<ScreenshotNumber> {
    let screenshot = u8::try_from(number).ok().and_then(ScreenshotNumber::new);
    if screenshot.is_none() {
        log::error!("The window named screenshot {number}, which does not exist");
    }
    screenshot
}
