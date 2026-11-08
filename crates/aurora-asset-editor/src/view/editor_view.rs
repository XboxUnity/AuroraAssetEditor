use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::{AssetSlot, SCREENSHOT_COUNT, ScreenshotNumber, TextureHeader};
use aurora_image::ImageTarget;
use aurora_ui::{EditorModel, MainWindow, PreviewArea, ScreenshotTile, SlotView};
use slint::ComponentHandle;

use crate::app::App;
use crate::editor::{AssetDocument, SlotPreview, texture_caption};

use super::shell_view::{Refresh, action_runner};
use super::{
    ImageCache, PictureId, Placeholder, PlaceholderArt, Previews, ScreenshotTileContent,
    ScreenshotTiles, ShownPicture, slint_image,
};

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

/// The images that make the editor's pictures: each slot's pixels as loaded, the placeholder art, and their
/// resized copies.
pub(super) struct EditorPictures<'a> {
    pub originals: &'a mut ImageCache,
    pub placeholders: &'a mut PlaceholderArt,
    pub previews: &'a mut Previews,
}

pub(super) fn render_editor(
    window: &MainWindow,
    app: &App,
    pictures: &mut EditorPictures<'_>,
    tiles: &mut ScreenshotTiles,
) {
    let editor = window.global::<EditorModel>();
    let pages = app.pages();
    editor.set_cover(slot_view(
        &pages.cover,
        AssetSlot::Cover,
        PreviewArea::Cover,
        pictures,
    ));
    editor.set_background(slot_view(
        &pages.background,
        AssetSlot::Background,
        PreviewArea::Background,
        pictures,
    ));
    editor.set_icon(slot_view(
        &pages.icon_banner,
        AssetSlot::Icon,
        PreviewArea::Icon,
        pictures,
    ));
    editor.set_banner(slot_view(
        &pages.icon_banner,
        AssetSlot::Banner,
        PreviewArea::Banner,
        pictures,
    ));

    let screenshots = &pages.screenshots;
    let selected = app.selected_screenshot();
    let mut screenshot = slot_view(
        screenshots,
        AssetSlot::Screenshot(selected),
        PreviewArea::Screenshot,
        pictures,
    );
    screenshot.caption = format!(
        "{} · {} of {SCREENSHOT_COUNT}",
        screenshot.caption,
        screenshots.screenshot_count()
    )
    .into();
    editor.set_screenshot(screenshot);
    editor.set_selected_screenshot(i32::from(selected.get()));
    let position_count = shown_position_count(screenshots);
    editor.set_screenshot_position_count(i32::try_from(position_count).unwrap_or(i32::MAX));
    tiles.show(
        tile_contents(screenshots, position_count, selected, pictures),
        screenshot_tile,
    );
    editor.set_drop_hover(app.drop_hover());
}

fn slot_view(
    document: &AssetDocument,
    slot: AssetSlot,
    area: PreviewArea,
    pictures: &mut EditorPictures<'_>,
) -> SlotView {
    let preview = document.preview(slot);
    let header = document.texture_header(slot);
    let picture = slot_picture(document, slot, area, pictures);
    let unsupported = matches!(preview, SlotPreview::Unsupported(_));
    // Only an empty slot shows its art, so only then is the art resized.
    let placeholder = match (&picture, unsupported) {
        (None, false) => Placeholder::of_slot(slot)
            .and_then(|placeholder| pictures.placeholders.show(placeholder, pictures.previews)),
        _ => None,
    };
    SlotView {
        slot_index: slot_index(slot),
        title: slot_title(slot).into(),
        caption: slot_caption(slot, header).into(),
        has_image: picture.is_some(),
        image: picture.map(|picture| picture.image).unwrap_or_default(),
        placeholder: placeholder
            .map(|placeholder| placeholder.image)
            .unwrap_or_default(),
        unsupported,
        can_export: matches!(preview, SlotPreview::Pixels(_)),
        can_remove: header.is_some(),
    }
}

/// Positions 1 to the last filled one, then the "+" tile for the position after it while there is one.
fn tile_contents(
    screenshots: &AssetDocument,
    shown_count: usize,
    selected: ScreenshotNumber,
    pictures: &mut EditorPictures<'_>,
) -> Vec<ScreenshotTileContent> {
    let mut content = |number: ScreenshotNumber, is_next_free: bool| {
        let slot = AssetSlot::Screenshot(number);
        let picture = if is_next_free {
            None
        } else {
            slot_picture(screenshots, slot, PreviewArea::Thumbnail, pictures)
        };
        ScreenshotTileContent {
            number,
            picture,
            filled: is_filled(screenshots, number),
            selected: number == selected,
            is_next_free,
        }
    };
    let mut contents: Vec<ScreenshotTileContent> = ScreenshotNumber::ALL[..shown_count]
        .iter()
        .map(|number| content(*number, false))
        .collect();
    if let Some(next_free) = ScreenshotNumber::ALL.get(shown_count) {
        contents.push(content(*next_free, true));
    }
    contents
}

fn screenshot_tile(content: &ScreenshotTileContent) -> ScreenshotTile {
    ScreenshotTile {
        number: i32::from(content.number.get()),
        image: content
            .picture
            .as_ref()
            .map(|picture| picture.image.clone())
            .unwrap_or_default(),
        filled: content.filled,
        selected: content.selected,
        is_next_free: content.is_next_free,
    }
}

/// Positions 1 to the last filled one; 0 when no screenshot is filled.
fn shown_position_count(screenshots: &AssetDocument) -> usize {
    ScreenshotNumber::ALL
        .iter()
        .rposition(|number| is_filled(screenshots, *number))
        .map_or(0, |last| last + 1)
}

fn is_filled(screenshots: &AssetDocument, number: ScreenshotNumber) -> bool {
    screenshots
        .texture_header(AssetSlot::Screenshot(number))
        .is_some()
}

/// The slot's picture as `area` shows it; `None` while the slot has no pixels to show.
fn slot_picture(
    document: &AssetDocument,
    slot: AssetSlot,
    area: PreviewArea,
    pictures: &mut EditorPictures<'_>,
) -> Option<ShownPicture> {
    let SlotPreview::Pixels(pixels) = document.preview(slot) else {
        return None;
    };
    let revision = document.revision(slot);
    let originals = &mut *pictures.originals;
    let original = || originals.image(slot, revision, || slint_image(pixels));
    let id = PictureId::Slot { slot, revision };
    Some(pictures.previews.show(id, area, pixels, original))
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
