use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use aurora_asset::{AssetFile, AssetSlot, Compression, ScreenshotNumber, TextureHeader, encode};
use aurora_asset_editor::app::{App, Page};
use aurora_asset_editor::editor::SlotPreview;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::MainWindowView;
use aurora_ui::{EditorModel, MainWindow, ScreenshotTile};
use editor_test_support::{AppHarness, HeadlessUi, asset_file_with};
use i_slint_backend_testing::ElementHandle;
use image::{Rgba, RgbaImage};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, Model, SharedString};
use tempfile::TempDir;

const DARK_SLOT_BACKGROUND: Rgba<u8> = Rgba([0x26, 0x2a, 0x31, 0xff]);
const LIGHT_SLOT_BACKGROUND: Rgba<u8> = Rgba([0xff, 0xff, 0xff, 0xff]);
const DARK_WINDOW_BACKGROUND: Rgba<u8> = Rgba([0x1b, 0x1d, 0x22, 0xff]);
const LIGHT_WINDOW_BACKGROUND: Rgba<u8> = Rgba([0xf5, 0xf6, 0xf8, 0xff]);
const SLOT_FILLED_BORDER: Rgba<u8> = Rgba([0x3b, 0x82, 0xf6, 0xff]);
const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
const GREEN: Rgba<u8> = Rgba([0, 255, 0, 255]);
const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);
const COLOUR_TOLERANCE: u8 = 24;
/// The placeholder art must differ this much from the slot in every channel to read as art.
const MIN_PLACEHOLDER_CONTRAST: u8 = 10;
const TILED_FLAG_BYTE: usize = 7 * 4;
const TILED_FLAG: u8 = 0x80;
/// The icon's art is a square in a wide slot, with a transparent cross.
const MIN_ART_SHARE: f32 = 0.1;
/// Inside the slot, clear of the dashed outline and the rounded corner.
const SLOT_INSET: f32 = 8.0;
/// The second pixel column of a slot: past a filled slot's 1 px border, inside the 2 px drop highlight.
const DROP_HIGHLIGHT_PROBE: f32 = 1.0;
const ASSET_PAGES: [Page; 4] = [
    Page::Cover,
    Page::Background,
    Page::IconBanner,
    Page::Screenshots,
];
const MINIMUM_WINDOW: (f32, f32) = (760.0, 500.0);
/// Longer than the wheel's smooth-scroll animation.
const WHEEL_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
const WHEEL_STEP: f32 = -60.0;
const MAX_WHEEL_STEPS: usize = 40;
/// Set to a folder to keep the rendered pages as PNG files for a look.
const SNAPSHOT_DIR_VARIABLE: &str = "AURORA_UI_SNAPSHOT_DIR";

struct Ui {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Ui {
    fn start(harness: &mut AppHarness) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window);
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let started = Ui { ui, view, app };
        started.render();
        started
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }

    fn act(&self, action: impl FnOnce(&mut App)) {
        action(&mut self.app.borrow_mut());
        self.render();
    }

    fn snapshot(&self) -> RgbaImage {
        self.ui.snapshot(self.view.window().window())
    }

    fn elements(&self, id: &str) -> Vec<ElementHandle> {
        ElementHandle::find_by_element_id(self.view.window(), id).collect()
    }

    fn element(&self, id: &str) -> ElementHandle {
        self.elements(id)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the window shows {id}"))
    }

    fn model(&self) -> EditorModel<'_> {
        self.view.window().global::<EditorModel>()
    }

    fn tiles(&self) -> Vec<ScreenshotTile> {
        self.model().get_screenshot_tiles().iter().collect()
    }

    fn enabled(&self, id: &str) -> bool {
        self.element(id)
            .accessible_enabled()
            .expect("buttons report whether they are enabled")
    }

    fn dispatch(&self, event: WindowEvent) {
        self.view.window().window().dispatch_event(event);
    }

    fn press_key(&self, key: Key) {
        let text = SharedString::from(key);
        self.dispatch(WindowEvent::KeyPressed { text: text.clone() });
        self.dispatch(WindowEvent::KeyReleased { text });
        settle();
    }

    /// The thumbnail with this accessible label, while any part of it is in view.
    fn thumb(&self, label: &str) -> Option<ElementHandle> {
        self.elements("ScreenshotStrip::thumb")
            .into_iter()
            .find(|thumb| thumb.accessible_label().as_deref() == Some(label))
    }

    /// Whether the whole thumbnail lies inside the strip's visible part and the window.
    fn thumb_in_view(&self, label: &str) -> bool {
        let Some(thumb) = self.thumb(label) else {
            return false;
        };
        let scroller = self.element("ScreenshotStrip::scroller");
        let left = scroller.absolute_position().x;
        // Scale factor 1: physical and logical pixels are the same.
        let window_width = self.view.window().window().size().width as f32;
        let right = (left + scroller.size().width).min(window_width);
        let thumb_left = thumb.absolute_position().x;
        thumb_left >= left && thumb_left + thumb.size().width <= right
    }
}

/// Runs the strip's scroll animation and change handlers to their end.
fn settle() {
    i_slint_backend_testing::mock_elapsed_time(WHEEL_SETTLE);
}

fn harness_with_theme(theme: ThemeChoice) -> AppHarness {
    let mut harness = AppHarness::new();
    harness.app().set_theme(theme);
    harness
}

fn solid(colour: Rgba<u8>) -> RgbaImage {
    RgbaImage::from_pixel(8, 8, colour)
}

fn write_asset(folder: &TempDir, name: &str, mut file: AssetFile) -> PathBuf {
    let path = folder.path().join(name);
    std::fs::write(&path, file.to_bytes()).expect("the fixture can be written");
    path
}

fn load(harness: &mut AppHarness, path: PathBuf) {
    harness.app().open_paths(vec![path]);
    harness.run_jobs();
}

fn load_entries(
    harness: &mut AppHarness,
    name: &str,
    entries: &[(AssetSlot, RgbaImage, Compression)],
) {
    let folder = TempDir::new().expect("tests need a temp folder");
    let path = write_asset(&folder, name, asset_file_with(entries));
    load(harness, path);
}

fn load_cover(harness: &mut AppHarness, colour: Rgba<u8>) {
    let cover = RgbaImage::from_pixel(900, 600, colour);
    load_entries(
        harness,
        "GC4D5307E6.asset",
        &[(AssetSlot::Cover, cover, Compression::Dxt)],
    );
}

fn load_every_asset_page(harness: &mut AppHarness) {
    load_cover(harness, RED);
    load_entries(
        harness,
        "BK4D5307E6.asset",
        &[(AssetSlot::Background, solid(GREEN), Compression::None)],
    );
    load_entries(
        harness,
        "GL4D5307E6.asset",
        &[
            (AssetSlot::Icon, solid(BLUE), Compression::None),
            (AssetSlot::Banner, solid(RED), Compression::None),
        ],
    );
    load_screenshots(harness, &[(1, RED), (2, GREEN), (3, BLUE)]);
}

fn load_screenshots(harness: &mut AppHarness, shots: &[(u8, Rgba<u8>)]) {
    let entries: Vec<_> = shots
        .iter()
        .map(|(number, colour)| {
            let slot = AssetSlot::screenshot(*number).expect("a screenshot position");
            (slot, solid(*colour), Compression::None)
        })
        .collect();
    load_entries(harness, "SS4D5307E6.asset", &entries);
}

fn tiled_cover() -> AssetFile {
    let mut texture =
        encode(&solid(Rgba([1, 2, 3, 255])), Compression::Dxt).expect("an 8x8 image encodes");
    let mut header = texture.header.to_bytes();
    header[TILED_FLAG_BYTE] |= TILED_FLAG;
    texture.header = TextureHeader::from_bytes(header);
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture));
    file
}

fn write_png(folder: &TempDir, name: &str, colour: Rgba<u8>) -> PathBuf {
    let path = folder.path().join(name);
    solid(colour).save(&path).expect("the PNG can be written");
    path
}

fn centre(element: &ElementHandle) -> LogicalPosition {
    let position = element.absolute_position();
    let size = element.size();
    LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    )
}

/// The share of the element's pixels that differ from the empty slot's background.
fn art_share(image: &RgbaImage, element: &ElementHandle) -> f32 {
    let origin = element.absolute_position();
    let size = element.size();
    let area = image::imageops::crop_imm(
        image,
        origin.x as u32,
        origin.y as u32,
        size.width as u32,
        size.height as u32,
    )
    .to_image();
    let art = area
        .pixels()
        .filter(|pixel| **pixel != DARK_SLOT_BACKGROUND)
        .count();
    art as f32 / area.pixels().len() as f32
}

fn pixel_at(image: &RgbaImage, position: LogicalPosition) -> Rgba<u8> {
    *image.get_pixel(position.x as u32, position.y as u32)
}

fn near(pixel: Rgba<u8>, expected: Rgba<u8>) -> bool {
    pixel
        .0
        .iter()
        .zip(expected.0)
        .all(|(actual, wanted)| actual.abs_diff(wanted) <= COLOUR_TOLERANCE)
}

/// The colour half way between `top` and `bottom`: `top` drawn at half opacity over `bottom`.
fn half_way(top: Rgba<u8>, bottom: Rgba<u8>) -> Rgba<u8> {
    Rgba(std::array::from_fn(|channel| {
        let sum = u16::from(top.0[channel]) + u16::from(bottom.0[channel]);
        u8::try_from(sum / 2).expect("the mean of two bytes is a byte")
    }))
}

fn keep_snapshot(image: &RgbaImage, name: &str) {
    if let Some(folder) = std::env::var_os(SNAPSHOT_DIR_VARIABLE) {
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).expect("the snapshot folder can be created");
        image
            .save(folder.join(format!("{name}.png")))
            .expect("the snapshot can be written");
    }
}

fn screenshot_preview(app: &App, number: u8) -> SlotPreview {
    let slot = AssetSlot::screenshot(number).expect("a screenshot position");
    app.pages().screenshots.preview(slot).clone()
}

fn screenshot_header(app: &App, number: u8) -> Option<TextureHeader> {
    let slot = AssetSlot::screenshot(number).expect("a screenshot position");
    app.pages().screenshots.texture_header(slot).cloned()
}

#[test]
fn empty_pages_show_placeholder_art() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    for page in ASSET_PAGES {
        ui.act(|app| app.show_page(page));
        let snapshot = ui.snapshot();
        if page == Page::Cover {
            let pixel = pixel_at(&snapshot, centre(&ui.element("ImagePanel::slot")));
            assert_ne!(
                pixel, DARK_SLOT_BACKGROUND,
                "the cover slot shows art at its centre"
            );
            assert_ne!(pixel.0[3], 0, "the cover slot centre is not transparent");
        }
        for art in ui.elements("DropSlot::placeholder-art") {
            assert!(
                art_share(&snapshot, &art) > MIN_ART_SHARE,
                "{page:?} shows its placeholder art"
            );
        }
        assert_eq!(
            ui.element("DropSlot::drop-hint")
                .accessible_label()
                .as_deref(),
            Some("Drop PNG / JPG / WebP / BMP / DDS / .asset here")
        );
    }
}

#[test]
fn loaded_cover_shows_caption_and_buttons() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_cover(&mut harness, RED);
    harness.app().show_page(Page::Cover);
    let ui = Ui::start(&mut harness);

    let cover = ui.model().get_cover();
    assert_eq!(cover.title, "Cover");
    assert_eq!(cover.caption, "900 x 600 · DXT");
    assert!(cover.has_image && cover.can_export && cover.can_remove);
    assert!(!cover.unsupported);
    for button in [
        "ImagePanel::replace-button",
        "ImagePanel::export-button",
        "ImagePanel::remove-button",
    ] {
        assert!(ui.enabled(button), "{button} is enabled");
    }
    assert!(ui.elements("ImagePanel::add-button").is_empty());
    let snapshot = ui.snapshot();
    let slot_centre = centre(&ui.element("ImagePanel::slot"));
    assert!(
        near(pixel_at(&snapshot, slot_centre), RED),
        "the preview fills the slot"
    );
}

#[test]
fn cover_buttons_call_the_app_with_the_cover_slot() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_cover(&mut harness, RED);
    harness.app().show_page(Page::Cover);
    let folder = TempDir::new().expect("tests need a temp folder");
    let exported = folder.path().join("cover.png");
    harness.dialogs().push_save(exported.clone());
    let ui = Ui::start(&mut harness);

    ui.element("ImagePanel::export-button")
        .mock_single_click(PointerEventButton::Left);
    assert!(exported.is_file(), "Export wrote the cover");

    ui.element("ImagePanel::remove-button")
        .mock_single_click(PointerEventButton::Left);
    assert!(
        ui.app
            .borrow()
            .pages()
            .cover
            .texture_header(AssetSlot::Cover)
            .is_none()
    );
    assert_eq!(ui.model().get_cover().caption, "900 x 600 · empty");
}

#[test]
fn unsupported_entry_shows_label() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let folder = TempDir::new().expect("tests need a temp folder");
    load(
        &mut harness,
        write_asset(&folder, "GC4D5307E6.asset", tiled_cover()),
    );
    harness.app().show_page(Page::Cover);
    let ui = Ui::start(&mut harness);

    let cover = ui.model().get_cover();
    assert!(cover.unsupported && !cover.has_image);
    assert_eq!(
        cover.caption,
        "8 x 8 · Unsupported texture format (kept as is)"
    );
    assert!(!cover.can_export && cover.can_remove);
    assert_eq!(
        ui.element("DropSlot::unsupported-label")
            .accessible_label()
            .as_deref(),
        Some("Unsupported texture format (kept as is)")
    );
    assert!(ui.elements("DropSlot::placeholder-art").is_empty());
}

#[test]
fn export_and_remove_disabled_when_empty() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);

    let cover = ui.model().get_cover();
    assert_eq!(cover.caption, "900 x 600 · empty");
    assert!(!cover.has_image && !cover.can_export && !cover.can_remove);
    assert!(ui.enabled("ImagePanel::replace-button"));
    assert!(!ui.enabled("ImagePanel::export-button"));
    assert!(!ui.enabled("ImagePanel::remove-button"));
    for (view, caption) in [
        (ui.model().get_background(), "1280 x 720 · empty"),
        (ui.model().get_icon(), "64 x 64 · empty"),
        (ui.model().get_banner(), "420 x 96 · empty"),
    ] {
        assert_eq!(view.caption, caption);
        assert!(!view.can_export && !view.can_remove);
    }
}

#[test]
fn screenshot_header_names_slot_and_count() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED), (2, GREEN), (3, BLUE)]);
    let ui = Ui::start(&mut harness);

    ui.elements("ScreenshotStrip::thumb")[1].mock_single_click(PointerEventButton::Left);
    let screenshot = ui.model().get_screenshot();
    assert_eq!(screenshot.title, "Screenshot 2");
    assert_eq!(screenshot.caption, "8 x 8 · ARGB · 3 of 20");
    assert!(ui.enabled("ImagePanel::add-button"));

    ui.act(|app| app.select_screenshot(ScreenshotNumber::new(5).expect("position 5")));
    let screenshot = ui.model().get_screenshot();
    assert_eq!(screenshot.title, "Screenshot 5");
    assert_eq!(screenshot.caption, "1000 x 562 · empty · 3 of 20");
    assert!(!screenshot.can_export && !screenshot.can_remove);
}

#[test]
fn screenshot_strip_shows_filled_slots_and_next_free() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED), (3, BLUE)]);
    let ui = Ui::start(&mut harness);

    let tiles = ui.tiles();
    let shape: Vec<_> = tiles
        .iter()
        .map(|tile| (tile.number, tile.filled, tile.is_next_free))
        .collect();
    assert_eq!(
        shape,
        [
            (1, true, false),
            (2, false, false),
            (3, true, false),
            (4, false, true)
        ]
    );
    assert!(tiles[0].selected, "position 1 is selected at start");
    assert_eq!(ui.elements("ScreenshotStrip::thumb").len(), 4);
    assert_eq!(ui.model().get_screenshot_position_count(), 3);
    assert_eq!(
        ui.element("ScreenshotStrip::hint")
            .accessible_label()
            .as_deref(),
        Some("drag to reorder")
    );

    let all: Vec<_> = (1..=20).map(|number| (number, GREEN)).collect();
    load_screenshots(&mut harness, &all);
    ui.render();
    let tiles = ui.tiles();
    assert_eq!(tiles.len(), 20, "no + tile once every position is filled");
    assert_eq!(ui.model().get_screenshot_position_count(), 20);
    assert!(tiles.iter().all(|tile| tile.filled && !tile.is_next_free));
}

#[test]
fn empty_strip_shows_only_the_add_tile() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);

    let tiles = ui.tiles();
    assert_eq!(tiles.len(), 1);
    assert!(tiles[0].is_next_free && tiles[0].number == 1);
    assert_eq!(ui.model().get_screenshot_position_count(), 0);
    assert!(ui.elements("ScreenshotStrip::hint").is_empty());
}

#[test]
fn dragging_a_thumbnail_reorders_screenshots() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED), (2, GREEN), (3, BLUE)]);
    let ui = Ui::start(&mut harness);
    let first_preview = screenshot_preview(&ui.app.borrow(), 1);
    let first_header = screenshot_header(&ui.app.borrow(), 1);

    let thumbs = ui.elements("ScreenshotStrip::thumb");
    thumbs[0].mock_drag(centre(&thumbs[2]), PointerEventButton::Left);
    ui.render();

    let app = ui.app.borrow();
    assert_eq!(screenshot_preview(&app, 3), first_preview);
    assert_eq!(screenshot_header(&app, 3), first_header);
    assert_eq!(app.selected_screenshot().get(), 3);
}

#[test]
fn dragging_shows_the_insertion_bar_and_a_short_move_selects() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED), (2, GREEN), (3, BLUE)]);
    let ui = Ui::start(&mut harness);
    let thumbs = ui.elements("ScreenshotStrip::thumb");
    let start = centre(&thumbs[0]);
    let target = centre(&thumbs[2]);
    let button = PointerEventButton::Left;

    ui.dispatch(WindowEvent::PointerMoved { position: start });
    ui.dispatch(WindowEvent::PointerPressed {
        position: start,
        button,
    });
    ui.dispatch(WindowEvent::PointerMoved { position: target });
    let bar = ui.element("ScreenshotStrip::insertion-bar");
    assert_eq!(bar.size().width, 2.0);
    let bar_x = bar.absolute_position().x;
    let third = thumbs[2].absolute_position().x + thumbs[2].size().width;
    assert!(
        bar_x > third && bar_x < third + 6.0,
        "the bar marks the gap after position 3"
    );
    let snapshot = ui.snapshot();
    let bar_centre = centre(&bar);
    assert!(
        near(pixel_at(&snapshot, bar_centre), SLOT_FILLED_BORDER),
        "the bar is the accent"
    );
    ui.dispatch(WindowEvent::PointerReleased {
        position: target,
        button,
    });
    assert!(ui.elements("ScreenshotStrip::insertion-bar").is_empty());

    let preview_two = screenshot_preview(&ui.app.borrow(), 2);
    let thumbs = ui.elements("ScreenshotStrip::thumb");
    let start = centre(&thumbs[1]);
    let nudge = LogicalPosition::new(start.x + 3.0, start.y);
    ui.dispatch(WindowEvent::PointerMoved { position: start });
    ui.dispatch(WindowEvent::PointerPressed {
        position: start,
        button,
    });
    ui.dispatch(WindowEvent::PointerMoved { position: nudge });
    assert!(ui.elements("ScreenshotStrip::insertion-bar").is_empty());
    ui.dispatch(WindowEvent::PointerReleased {
        position: nudge,
        button,
    });
    assert_eq!(
        screenshot_preview(&ui.app.borrow(), 2),
        preview_two,
        "no move"
    );
    assert_eq!(ui.app.borrow().selected_screenshot().get(), 2);
}

#[test]
fn drop_hover_highlights_page() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    let slot = ui.element("ImagePanel::slot");
    let top_edge = LogicalPosition::new(centre(&slot).x, slot.absolute_position().y);

    assert!(!near(
        pixel_at(&ui.snapshot(), top_edge),
        SLOT_FILLED_BORDER
    ));

    ui.act(|app| app.set_drop_hover(true));
    assert!(ui.model().get_drop_hover());
    assert_eq!(pixel_at(&ui.snapshot(), top_edge), SLOT_FILLED_BORDER);
}

#[test]
fn drop_hover_highlights_filled_slots_on_every_asset_page() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_every_asset_page(&mut harness);
    let ui = Ui::start(&mut harness);
    for page in ASSET_PAGES {
        ui.act(|app| {
            app.show_page(page);
            app.set_drop_hover(false);
        });
        let idle = ui.snapshot();
        ui.act(|app| app.set_drop_hover(true));
        let hovered = ui.snapshot();
        for slot in ui.elements("ImagePanel::slot") {
            // Inside the filled slot's own 1 px border, where the preview starts. The left edge, because the
            // Icon & Banner page's second slot starts half a pixel down.
            let inside_border = LogicalPosition::new(
                slot.absolute_position().x + DROP_HIGHLIGHT_PROBE,
                centre(&slot).y,
            );
            assert_ne!(
                pixel_at(&idle, inside_border),
                SLOT_FILLED_BORDER,
                "{page:?}: no highlight before the drag"
            );
            assert_eq!(
                pixel_at(&hovered, inside_border),
                SLOT_FILLED_BORDER,
                "{page:?}: the highlight shows over the preview"
            );
        }
    }
}

#[test]
fn editor_buttons_disabled_while_importing() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_cover(&mut harness, RED);
    harness.app().show_page(Page::Cover);
    let folder = TempDir::new().expect("tests need a temp folder");
    harness
        .dialogs()
        .push_files(vec![write_png(&folder, "new_cover.png", BLUE)]);
    let ui = Ui::start(&mut harness);
    let buttons = [
        "ImagePanel::replace-button",
        "ImagePanel::export-button",
        "ImagePanel::remove-button",
    ];

    ui.element("ImagePanel::replace-button")
        .mock_single_click(PointerEventButton::Left);
    assert!(ui.app.borrow().locks().editor, "the import is queued");
    for button in buttons {
        assert!(!ui.enabled(button), "{button} is disabled while importing");
    }

    harness.run_jobs();
    ui.render();
    for button in buttons {
        assert!(ui.enabled(button), "{button} is enabled again");
    }
    let snapshot = ui.snapshot();
    let slot_centre = centre(&ui.element("ImagePanel::slot"));
    assert!(
        near(pixel_at(&snapshot, slot_centre), BLUE),
        "the new cover is shown"
    );
}

#[test]
fn add_tile_looks_disabled_while_importing() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED)]);
    let folder = TempDir::new().expect("tests need a temp folder");
    harness
        .dialogs()
        .push_files(vec![write_png(&folder, "more.png", BLUE)]);
    let ui = Ui::start(&mut harness);
    let add_tile = ui.thumb("Add…").expect("the strip shows the Add tile");
    assert_eq!(add_tile.accessible_enabled(), Some(true));
    let plus = centre(&add_tile);
    let enabled = pixel_at(&ui.snapshot(), plus);

    ui.act(|app| app.add_screenshots());
    assert!(ui.app.borrow().locks().editor, "the import is queued");
    let add_tile = ui.thumb("Add…").expect("the strip shows the Add tile");
    assert_eq!(add_tile.accessible_enabled(), Some(false));
    let dimmed = pixel_at(&ui.snapshot(), plus);
    assert!(
        near(dimmed, half_way(enabled, DARK_WINDOW_BACKGROUND)),
        "the Add tile is drawn at half opacity like a disabled button: {enabled:?} became {dimmed:?}"
    );
}

#[test]
fn asset_pages_render_in_both_themes() {
    for (theme, slot_background, window_background, name) in [
        (
            ThemeChoice::Dark,
            DARK_SLOT_BACKGROUND,
            DARK_WINDOW_BACKGROUND,
            "dark",
        ),
        (
            ThemeChoice::Light,
            LIGHT_SLOT_BACKGROUND,
            LIGHT_WINDOW_BACKGROUND,
            "light",
        ),
    ] {
        for loaded in [false, true] {
            let mut harness = harness_with_theme(theme);
            if loaded {
                load_every_asset_page(&mut harness);
            }
            let ui = Ui::start(&mut harness);
            let state = if loaded { "loaded" } else { "empty" };
            for page in ASSET_PAGES {
                ui.act(|app| app.show_page(page));
                let snapshot = ui.snapshot();
                keep_snapshot(&snapshot, &format!("pages-{name}-{state}-{page:?}"));

                for slot in ui.elements("ImagePanel::slot") {
                    let origin = slot.absolute_position();
                    let inside = LogicalPosition::new(origin.x + SLOT_INSET, origin.y + SLOT_INSET);
                    let pixel = pixel_at(&snapshot, inside);
                    if loaded {
                        assert_ne!(
                            pixel, slot_background,
                            "{name} {page:?}: filled slots use the gradient"
                        );
                    } else {
                        assert_eq!(
                            pixel, slot_background,
                            "{name} {page:?}: empty slot background"
                        );
                    }
                }
                let below_header = LogicalPosition::new(
                    ui.element("ImagePanel::slot").absolute_position().x - 4.0,
                    ui.element("ImagePanel::slot").absolute_position().y + SLOT_INSET,
                );
                assert_eq!(
                    pixel_at(&snapshot, below_header),
                    window_background,
                    "{name} {page:?} body"
                );
            }
        }
    }
}

#[test]
fn placeholder_art_stands_out_in_both_themes() {
    for (theme, slot_background) in [
        (ThemeChoice::Dark, DARK_SLOT_BACKGROUND),
        (ThemeChoice::Light, LIGHT_SLOT_BACKGROUND),
    ] {
        let mut harness = harness_with_theme(theme);
        let ui = Ui::start(&mut harness);
        let pixel = pixel_at(&ui.snapshot(), centre(&ui.element("ImagePanel::slot")));
        for (art, slot) in pixel.0.iter().zip(slot_background.0).take(3) {
            assert!(
                art.abs_diff(slot) >= MIN_PLACEHOLDER_CONTRAST,
                "{theme:?}: placeholder {pixel:?} on slot {slot_background:?}"
            );
        }
    }
}

#[test]
fn thumbnails_neither_select_nor_drag_while_the_editor_is_locked() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_screenshots(&mut harness, &[(1, RED), (2, GREEN), (3, BLUE)]);
    let folder = TempDir::new().expect("tests need a temp folder");
    harness
        .dialogs()
        .push_files(vec![write_png(&folder, "more.png", BLUE)]);
    let ui = Ui::start(&mut harness);
    ui.act(|app| app.add_screenshots());
    assert!(ui.app.borrow().locks().editor, "the import is queued");
    let first_preview = screenshot_preview(&ui.app.borrow(), 1);

    let thumbs = ui.elements("ScreenshotStrip::thumb");
    thumbs[1].mock_single_click(PointerEventButton::Left);
    assert_eq!(
        ui.app.borrow().selected_screenshot().get(),
        1,
        "no selection"
    );

    let start = centre(&thumbs[0]);
    let target = centre(&thumbs[2]);
    let button = PointerEventButton::Left;
    ui.dispatch(WindowEvent::PointerMoved { position: start });
    ui.dispatch(WindowEvent::PointerPressed {
        position: start,
        button,
    });
    ui.dispatch(WindowEvent::PointerMoved { position: target });
    assert!(
        ui.elements("ScreenshotStrip::insertion-bar").is_empty(),
        "no drag"
    );
    ui.dispatch(WindowEvent::PointerReleased {
        position: target,
        button,
    });
    let app = ui.app.borrow();
    assert_eq!(screenshot_preview(&app, 1), first_preview, "no move");
    assert_eq!(app.selected_screenshot().get(), 1);
}

#[test]
fn new_asset_set_then_load_shows_the_new_image() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    load_cover(&mut harness, RED);
    harness.app().show_page(Page::Cover);
    let ui = Ui::start(&mut harness);
    let slot_centre = centre(&ui.element("ImagePanel::slot"));
    assert!(near(pixel_at(&ui.snapshot(), slot_centre), RED));

    // No render in between: the cover's revision must not come back to the one on screen.
    ui.app.borrow_mut().new_asset_set();
    load_cover(&mut harness, BLUE);
    ui.render();

    assert!(
        near(pixel_at(&ui.snapshot(), slot_centre), BLUE),
        "the new cover is shown"
    );
}

#[test]
fn screenshot_strip_scrolls_to_every_thumbnail() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let all: Vec<_> = (1..=20).map(|number| (number, GREEN)).collect();
    load_screenshots(&mut harness, &all);
    harness.app().show_page(Page::Screenshots);
    let ui = Ui::start(&mut harness);

    for (width, height) in [
        (
            HeadlessUi::WINDOW_SIZE.0 as f32,
            HeadlessUi::WINDOW_SIZE.1 as f32,
        ),
        MINIMUM_WINDOW,
    ] {
        ui.view
            .window()
            .window()
            .set_size(LogicalSize::new(width, height));
        ui.act(|app| app.select_screenshot(ScreenshotNumber::new(1).expect("position 1")));
        settle();
        assert!(
            ui.thumb_in_view("Screenshot 1"),
            "{width}: the first thumbnail is in view"
        );
        assert!(
            !ui.thumb_in_view("Screenshot 20"),
            "{width}: twenty do not fit at once"
        );
        assert!(
            !ui.elements("ScreenshotStrip::scroll-thumb").is_empty(),
            "{width}: the strip shows that it scrolls"
        );

        let strip = centre(&ui.element("ScreenshotStrip::scroller"));
        for _ in 0..MAX_WHEEL_STEPS {
            if ui.thumb_in_view("Screenshot 20") {
                break;
            }
            ui.dispatch(WindowEvent::PointerScrolled {
                position: strip,
                delta_x: 0.0,
                delta_y: WHEEL_STEP,
            });
            settle();
        }
        assert!(
            ui.thumb_in_view("Screenshot 20"),
            "{width}: the wheel reaches thumbnail 20"
        );
        keep_snapshot(
            &ui.snapshot(),
            &format!("pages-dark-twenty-screenshots-{width}"),
        );

        ui.act(|app| app.select_screenshot(ScreenshotNumber::new(2).expect("position 2")));
        settle();
        assert!(
            ui.thumb_in_view("Screenshot 2"),
            "{width}: selecting position 2 scrolls back"
        );
        ui.act(|app| app.select_screenshot(ScreenshotNumber::new(1).expect("position 1")));
        settle();
        assert!(ui.thumb_in_view("Screenshot 1"), "{width}: and position 1");
        // A click gives the strip the keyboard.
        ui.thumb("Screenshot 1")
            .expect("position 1 is in view")
            .mock_single_click(PointerEventButton::Left);
        ui.press_key(Key::End);
        ui.render();
        settle();
        assert_eq!(ui.app.borrow().selected_screenshot().get(), 20);
        assert!(
            ui.thumb_in_view("Screenshot 20"),
            "{width}: End selects and shows position 20"
        );
        ui.press_key(Key::LeftArrow);
        ui.render();
        assert_eq!(ui.app.borrow().selected_screenshot().get(), 19);
    }
}

#[test]
fn adding_a_screenshot_scrolls_it_into_view() {
    let mut harness = harness_with_theme(ThemeChoice::Dark);
    let shots: Vec<_> = (1..=15).map(|number| (number, GREEN)).collect();
    load_screenshots(&mut harness, &shots);
    harness.app().show_page(Page::Screenshots);
    let folder = TempDir::new().expect("tests need a temp folder");
    harness
        .dialogs()
        .push_files(vec![write_png(&folder, "more.png", BLUE)]);
    let ui = Ui::start(&mut harness);
    ui.act(|app| app.select_screenshot(ScreenshotNumber::new(1).expect("position 1")));
    settle();
    assert!(ui.thumb("Add…").is_none(), "the + tile is out of view");

    ui.act(|app| app.add_screenshots());
    harness.run_jobs();
    ui.render();
    settle();
    assert_eq!(ui.app.borrow().selected_screenshot().get(), 16);
    assert!(
        ui.thumb_in_view("Screenshot 16"),
        "the added screenshot is in view"
    );
}
