//! Pictures reach the window resized to the size they are shown at, so the software renderer, which would
//! scale them nearest-neighbour, draws them pixel for pixel.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use aurora_asset::{AssetKind, AssetSlot, Compression, RgbaImage};
use aurora_asset_editor::app::{App, Page};
use aurora_asset_editor::view::{MainWindowView, SPACE_SETTLE_TIME};
use aurora_image::preview_size;
use aurora_ui::{EditorModel, MainWindow};
use editor_test_support::{
    AppHarness, HeadlessUi, PreviewJobs, asset_file_with, settle_preview_spaces,
};
use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use image::Rgba;
use slint::{ComponentHandle, Image, LogicalSize};
use tempfile::TempDir;

/// Larger than the cover slot at the default window size, so it is shown smaller.
const COVER_SIZE: (u32, u32) = (900, 600);
/// Smaller than its slot, so it is shown larger.
const ICON_SIZE: (u32, u32) = (64, 64);
const BANNER_SIZE: (u32, u32) = (420, 96);
/// The old app's banner placeholder art is a pixel shorter than a banner.
const PLACEHOLDER_BANNER_SIZE: (u32, u32) = (420, 95);
/// Steps of a drag on the window's edge, each shorter than the wait for a space to settle.
const DRAG_SIZES: [(f32, f32); 4] = [
    (980.0, 660.0),
    (940.0, 640.0),
    (900.0, 620.0),
    (860.0, 600.0),
];
const DRAG_STEP: Duration = Duration::from_millis(30);
const _: () = assert!(DRAG_STEP.as_millis() < SPACE_SETTLE_TIME.as_millis());

struct Ui {
    _ui: HeadlessUi,
    view: MainWindowView,
    previews: PreviewJobs,
    app: Rc<RefCell<App>>,
}

impl Ui {
    fn start(harness: &mut AppHarness) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let previews = PreviewJobs::new();
        let view = MainWindowView::new(window, previews.runner());
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let started = Ui {
            _ui: ui,
            view,
            previews,
            app,
        };
        started.render();
        // The first page's empty slot shows its placeholder art, resized like any picture.
        assert_eq!(started.settle_and_resize(), 1, "the cover's art");
        started
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }

    fn show_page(&self, page: Page) {
        self.app.borrow_mut().show_page(page);
        self.render();
    }

    /// Lets the spaces settle and runs the resizing they ask for; returns how many pictures were resized.
    fn settle_and_resize(&self) -> usize {
        settle_preview_spaces();
        let resized = self.previews.run(&self.view);
        self.render();
        resized
    }

    fn model(&self) -> EditorModel<'_> {
        self.view.window().global::<EditorModel>()
    }

    /// The space a slot gives its picture: its preview element, at scale factor 1 in physical pixels.
    fn slot_space(&self, index: usize) -> (u32, u32) {
        let previews: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(self.view.window(), "DropSlot::preview").collect();
        let size = previews
            .get(index)
            .unwrap_or_else(|| panic!("the page has a slot {index}"))
            .size();
        (size.width.floor() as u32, size.height.floor() as u32)
    }

    /// The space an empty slot gives its placeholder art, like `slot_space`.
    fn placeholder_space(&self, index: usize) -> (u32, u32) {
        let arts: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(self.view.window(), "DropSlot::placeholder-art")
                .collect();
        let size = arts
            .get(index)
            .unwrap_or_else(|| panic!("the page has an empty slot {index}"))
            .size();
        (size.width.floor() as u32, size.height.floor() as u32)
    }

    /// The size the window draws each placeholder art at, in the order the page shows them.
    fn drawn_placeholder_sizes(&self) -> Vec<(u32, u32)> {
        ElementHandle::find_by_element_id(self.view.window(), "DropSlot::placeholder-art")
            .map(|art| {
                let picture = art
                    .query_descendants()
                    .match_id("PreviewImage::picture")
                    .find_first()
                    .expect("the art is drawn by a picture");
                let size = picture.size();
                (size.width.round() as u32, size.height.round() as u32)
            })
            .collect()
    }

    /// The size the window draws the picture of each slot at, in the order the page shows them.
    fn drawn_sizes(&self) -> Vec<(u32, u32)> {
        ElementHandle::find_by_element_id(self.view.window(), "PreviewImage::picture")
            .map(|picture| {
                let size = picture.size();
                (size.width.round() as u32, size.height.round() as u32)
            })
            .collect()
    }
}

fn size_of(image: &Image) -> (u32, u32) {
    let size = image.size();
    (size.width, size.height)
}

fn write_asset(folder: &Path, name: &str, entries: &[(AssetSlot, RgbaImage)]) -> PathBuf {
    let entries: Vec<_> = entries
        .iter()
        .map(|(slot, pixels)| (*slot, pixels.clone(), Compression::None))
        .collect();
    let path = folder.join(name);
    fs::write(&path, asset_file_with(&entries).to_bytes()).expect("the fixture can be written");
    path
}

/// Diagonal stripes, so a resize that drops or doubles pixels would change them.
fn striped(size: (u32, u32)) -> RgbaImage {
    RgbaImage::from_fn(size.0, size.1, |x, y| {
        let band = ((x + y) / 4 % 2 * 200) as u8;
        Rgba([band, 255 - band, 90, 255])
    })
}

fn load(harness: &mut AppHarness, ui: &Ui, path: PathBuf) {
    ui.app.borrow_mut().open_paths(vec![path]);
    harness.run_jobs();
    ui.render();
}

fn load_icon_and_banner(harness: &mut AppHarness, ui: &Ui, folder: &Path) -> PathBuf {
    let path = write_asset(
        folder,
        "GL4D5307E6.asset",
        &[
            (AssetSlot::Icon, striped(ICON_SIZE)),
            (AssetSlot::Banner, striped(BANNER_SIZE)),
        ],
    );
    load(harness, ui, path.clone());
    path
}

#[test]
fn pictures_reach_the_window_at_the_size_they_are_shown() {
    let folder = TempDir::new().expect("tests need a temp folder");
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);
    let cover = write_asset(
        folder.path(),
        "GC4D5307E6.asset",
        &[(AssetSlot::Cover, striped(COVER_SIZE))],
    );
    load(&mut harness, &ui, cover);
    load_icon_and_banner(&mut harness, &ui, folder.path());

    ui.show_page(Page::Cover);
    ui.settle_and_resize();
    let cover_size = preview_size(COVER_SIZE, ui.slot_space(0));
    assert!(cover_size.0 < COVER_SIZE.0, "the cover is shown smaller");
    assert_eq!(size_of(&ui.model().get_cover().image), cover_size);
    assert_eq!(ui.drawn_sizes(), vec![cover_size], "drawn pixel for pixel");

    ui.show_page(Page::IconBanner);
    ui.settle_and_resize();
    let icon_size = preview_size(ICON_SIZE, ui.slot_space(0));
    let banner_size = preview_size(BANNER_SIZE, ui.slot_space(1));
    assert!(icon_size.0 > ICON_SIZE.0, "the icon is shown larger");
    assert_eq!(size_of(&ui.model().get_icon().image), icon_size);
    assert_eq!(size_of(&ui.model().get_banner().image), banner_size);
    assert_eq!(ui.drawn_sizes(), vec![icon_size, banner_size]);
}

#[test]
fn a_resize_of_the_window_resizes_each_picture_once_after_it_settles() {
    let folder = TempDir::new().expect("tests need a temp folder");
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);
    load_icon_and_banner(&mut harness, &ui, folder.path());
    ui.show_page(Page::IconBanner);
    assert_eq!(ui.settle_and_resize(), 2, "the icon and the banner");
    let before = ui.slot_space(0);

    for (width, height) in DRAG_SIZES {
        ui.view
            .window()
            .window()
            .set_size(LogicalSize::new(width, height));
        ui.render();
        mock_elapsed_time(DRAG_STEP);
        assert_eq!(
            ui.previews.queued(),
            0,
            "nothing is resized during the drag"
        );
    }
    assert_eq!(
        ui.settle_and_resize(),
        2,
        "one new copy of each picture once the size holds still"
    );

    assert_ne!(ui.slot_space(0), before, "the slots have a new size");
    let icon_size = preview_size(ICON_SIZE, ui.slot_space(0));
    let banner_size = preview_size(BANNER_SIZE, ui.slot_space(1));
    assert_eq!(size_of(&ui.model().get_icon().image), icon_size);
    assert_eq!(size_of(&ui.model().get_banner().image), banner_size);
}

#[test]
fn placeholder_art_reaches_the_window_at_the_size_it_is_shown() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);

    ui.show_page(Page::IconBanner);
    assert_eq!(ui.settle_and_resize(), 2, "the icon's and the banner's art");

    let icon_size = preview_size(ICON_SIZE, ui.placeholder_space(0));
    assert!(icon_size.0 > ICON_SIZE.0, "the icon's art is shown larger");
    assert_eq!(size_of(&ui.model().get_icon().placeholder), icon_size);
    let banner_art = size_of(&ui.model().get_banner().placeholder);
    assert_eq!(
        banner_art,
        preview_size(PLACEHOLDER_BANNER_SIZE, ui.placeholder_space(1))
    );
    assert_eq!(
        ui.drawn_placeholder_sizes(),
        vec![icon_size, banner_art],
        "drawn pixel for pixel"
    );
}

#[test]
fn a_failed_resize_is_asked_for_again_once_per_size() {
    let folder = TempDir::new().expect("tests need a temp folder");
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);
    load_icon_and_banner(&mut harness, &ui, folder.path());
    ui.show_page(Page::IconBanner);
    settle_preview_spaces();

    assert_eq!(ui.previews.fail(&ui.view), 2, "the icon and the banner");
    ui.render();
    assert_eq!(ui.previews.fail(&ui.view), 2, "asked for again");
    ui.render();
    ui.render();
    assert_eq!(
        ui.previews.queued(),
        0,
        "a second failure at the same size is not retried"
    );
    assert_eq!(
        size_of(&ui.model().get_icon().image),
        ICON_SIZE,
        "as loaded"
    );

    let (width, height) = DRAG_SIZES[0];
    ui.view
        .window()
        .window()
        .set_size(LogicalSize::new(width, height));
    ui.render();
    assert_eq!(ui.settle_and_resize(), 2, "a new size is asked for");
    let icon_size = preview_size(ICON_SIZE, ui.slot_space(0));
    assert_eq!(size_of(&ui.model().get_icon().image), icon_size);
}

#[test]
fn resized_pictures_leave_the_saved_asset_unchanged() {
    let folder = TempDir::new().expect("tests need a temp folder");
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);
    let source = load_icon_and_banner(&mut harness, &ui, folder.path());
    ui.show_page(Page::IconBanner);
    assert_eq!(ui.settle_and_resize(), 2);
    let target = folder.path().join("saved.asset");

    harness.dialogs().push_save(target.clone());
    ui.app.borrow_mut().save_kind(AssetKind::IconBanner);

    assert_eq!(
        fs::read(target).expect("the asset was saved"),
        fs::read(source).expect("the loaded asset is still there")
    );
}
