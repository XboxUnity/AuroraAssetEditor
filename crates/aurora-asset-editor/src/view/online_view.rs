use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::RgbaImage;
use aurora_online::CoverSource;
use aurora_ui::{MainWindow, OnlineModel, ResultTileView};
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::app::App;
use crate::online::{OnlineState, ResultTile, SearchMode, TileImage};

use super::shell_view::{Refresh, action_runner};
use super::{OnlineImageCache, ResultImage, ResultImageKey};

const EMPTY_GRID_TEXT: &str = "Search XboxUnity or the Internet Archive for covers";
const TITLE_ID_PLACEHOLDER: &str = "TitleID";
const KEYWORDS_PLACEHOLDER: &str = "Keywords";
const SOURCES: [CoverSource; 2] = [CoverSource::XboxUnity, CoverSource::InternetArchive];
const MODES: [SearchMode; 2] = [SearchMode::TitleId, SearchMode::Keywords];
const NO_SELECTION: i32 = -1;

/// Connects the Online page's search row, grid and result panel to `app`.
pub(super) fn connect_online(window: &MainWindow, app: &Rc<RefCell<App>>, refresh: &Refresh) {
    let online = window.global::<OnlineModel>();

    let act = action_runner(app, refresh);
    online.on_set_source(move |index| {
        if let Some(source) = choice_of(&SOURCES, index, "source") {
            act(&mut |app| app.set_online_source(source));
        }
    });

    let act = action_runner(app, refresh);
    online.on_set_mode(move |index| {
        if let Some(mode) = choice_of(&MODES, index, "search mode") {
            act(&mut |app| app.set_search_mode(mode));
        }
    });

    let act = action_runner(app, refresh);
    online.on_edit_search(move |text| act(&mut |app| app.edit_search_text(&text)));

    let act = action_runner(app, refresh);
    online.on_search(move || act(&mut |app| app.search_online()));

    let act = action_runner(app, refresh);
    online.on_select(move |index| {
        if let Some(index) = result_index_of(index) {
            act(&mut |app| app.select_result(index));
        }
    });

    let act = action_runner(app, refresh);
    online.on_use_as_cover(move || act(&mut |app| app.use_result_as_cover()));

    let act = action_runner(app, refresh);
    online.on_save_image(move || act(&mut |app| app.save_result_image()));
}

pub(super) fn render_online(window: &MainWindow, app: &App, images: &mut OnlineImageCache) {
    let online = window.global::<OnlineModel>();
    let state = app.online();
    online.set_source_index(position_of(&SOURCES, state.source));
    online.set_mode_index(position_of(&MODES, state.mode));
    online.set_keywords_enabled(state.source != CoverSource::InternetArchive);
    online.set_search_text(state.search_text().into());
    online.set_search_placeholder(placeholder(state.mode).into());
    online.set_tiles(result_tiles(state, images));
    online.set_searching(app.locks().online_search);
    online.set_empty_text(empty_text(state, app.locks().online_search).into());
    render_selection(&online, state, app.locks().use_result, images);
}

fn render_selection(
    online: &OnlineModel,
    state: &OnlineState,
    use_locked: bool,
    images: &mut OnlineImageCache,
) {
    let selected = state.selected_tile();
    let full = selected.map(|tile| &tile.full);
    let full_pixels = match full {
        Some(TileImage::Loaded { pixels, .. }) => Some(pixels),
        _ => None,
    };
    online.set_selected(state.selected.map_or(NO_SELECTION, row_number));
    online.set_selected_label(
        selected
            .map(|tile| tile.result.label.as_str())
            .unwrap_or_default()
            .into(),
    );
    online.set_selected_size(
        full_pixels
            .map(|pixels| format!("{}x{}", pixels.width(), pixels.height()))
            .unwrap_or_default()
            .into(),
    );
    let full_image = match (state.selected, full_pixels) {
        (Some(index), Some(pixels)) => {
            let key = image_key(state, index, ResultImage::Full);
            images.image(key, || slint_image(pixels))
        }
        _ => Image::default(),
    };
    online.set_selected_image(full_image);
    online.set_selected_has_image(full_pixels.is_some());
    online.set_selected_failed(matches!(full, Some(TileImage::Failed)));
    online.set_can_use(full_pixels.is_some() && !use_locked);
    online.set_can_save(full_pixels.is_some());
}

fn result_tiles(state: &OnlineState, images: &mut OnlineImageCache) -> ModelRc<ResultTileView> {
    let tiles: Vec<ResultTileView> = state
        .results
        .iter()
        .enumerate()
        .map(|(index, tile)| {
            let thumbnail = match &tile.thumbnail {
                TileImage::Loaded { pixels, .. } => {
                    let key = image_key(state, index, ResultImage::Thumbnail);
                    Some(images.image(key, || slint_image(pixels)))
                }
                TileImage::Pending | TileImage::Failed => None,
            };
            result_tile_view(tile, thumbnail, state.selected == Some(index))
        })
        .collect();
    ModelRc::new(VecModel::from(tiles))
}

fn result_tile_view(tile: &ResultTile, thumbnail: Option<Image>, selected: bool) -> ResultTileView {
    ResultTileView {
        label: tile.result.label.as_str().into(),
        has_image: thumbnail.is_some(),
        image: thumbnail.unwrap_or_default(),
        failed: matches!(tile.thumbnail, TileImage::Failed),
        selected,
    }
}

fn image_key(state: &OnlineState, index: usize, image: ResultImage) -> ResultImageKey {
    ResultImageKey {
        generation: state.generation(),
        index,
        image,
    }
}

/// The hint while the grid is empty, except while a search runs (the status line says so then).
fn empty_text(state: &OnlineState, searching: bool) -> &'static str {
    if state.results.is_empty() && !searching {
        EMPTY_GRID_TEXT
    } else {
        ""
    }
}

fn placeholder(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::TitleId => TITLE_ID_PLACEHOLDER,
        SearchMode::Keywords => KEYWORDS_PLACEHOLDER,
    }
}

fn slint_image(pixels: &RgbaImage) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
    );
    Image::from_rgba8(buffer)
}

fn position_of<T: PartialEq>(choices: &[T], value: T) -> i32 {
    choices
        .iter()
        .position(|choice| *choice == value)
        .map_or(NO_SELECTION, row_number)
}

fn row_number(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

fn choice_of<T: Copy>(choices: &[T], index: i32, what: &str) -> Option<T> {
    let choice = usize::try_from(index)
        .ok()
        .and_then(|index| choices.get(index))
        .copied();
    if choice.is_none() {
        log::error!("The window chose {what} {index}, which the Online page does not offer");
    }
    choice
}

fn result_index_of(index: i32) -> Option<usize> {
    let result = usize::try_from(index).ok();
    if result.is_none() {
        log::error!("The window selected result {index}, which does not exist");
    }
    result
}
