use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::RgbaImage;
use aurora_online::CoverSource;
use aurora_ui::{MainWindow, OnlineModel, PreviewArea, ResultTileView};
use slint::{ComponentHandle, Image};

use crate::app::App;
use crate::online::{OnlineState, SearchMode, TileImage};

use super::shell_view::{Refresh, action_runner, slint_int};
use super::{
    OnlineImageCache, PictureId, Previews, ResultImage, ResultImageKey, ResultTileContent,
    ResultTiles, ShownPicture, slint_image,
};

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

/// The images that make the Online page's pictures: each result image as loaded, and their resized copies.
pub(super) struct OnlinePictures<'a> {
    pub originals: &'a mut OnlineImageCache,
    pub previews: &'a mut Previews,
}

pub(super) fn render_online(
    window: &MainWindow,
    app: &App,
    pictures: &mut OnlinePictures<'_>,
    tiles: &mut ResultTiles,
) {
    let online = window.global::<OnlineModel>();
    let state = app.online();
    online.set_source_index(position_of(&SOURCES, state.source));
    online.set_mode_index(position_of(&MODES, state.mode));
    online.set_keywords_enabled(state.source != CoverSource::InternetArchive);
    online.set_search_text(state.search_text().into());
    online.set_search_placeholder(placeholder(state.mode).into());
    tiles.show(result_contents(state, pictures), result_tile_view);
    online.set_searching(app.locks().online_search);
    online.set_empty_text(empty_text(state, app.locks().online_search).into());
    render_selection(&online, state, app.locks().use_result, pictures);
}

fn render_selection(
    online: &OnlineModel,
    state: &OnlineState,
    use_locked: bool,
    pictures: &mut OnlinePictures<'_>,
) {
    let selected = state.selected_tile();
    let full = selected.map(|tile| &tile.full);
    let full_pixels = match full {
        Some(TileImage::Loaded { pixels, .. }) => Some(pixels),
        _ => None,
    };
    online.set_selected(state.selected.map_or(NO_SELECTION, slint_int));
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
            result_picture(key, PreviewArea::ResultPreview, pixels, pictures).image
        }
        _ => Image::default(),
    };
    online.set_selected_image(full_image);
    online.set_selected_has_image(full_pixels.is_some());
    online.set_selected_failed(matches!(full, Some(TileImage::Failed)));
    online.set_can_use(full_pixels.is_some() && !use_locked);
    online.set_can_save(full_pixels.is_some());
}

fn result_contents(
    state: &OnlineState,
    pictures: &mut OnlinePictures<'_>,
) -> Vec<ResultTileContent> {
    state
        .results
        .iter()
        .enumerate()
        .map(|(index, tile)| ResultTileContent {
            label: tile.result.label.as_str().into(),
            thumbnail: match &tile.thumbnail {
                TileImage::Loaded { pixels, .. } => {
                    let key = image_key(state, index, ResultImage::Thumbnail);
                    Some(result_picture(key, PreviewArea::Result, pixels, pictures))
                }
                TileImage::Pending | TileImage::Failed => None,
            },
            failed: matches!(tile.thumbnail, TileImage::Failed),
            selected: state.selected == Some(index),
        })
        .collect()
}

fn result_tile_view(content: &ResultTileContent) -> ResultTileView {
    ResultTileView {
        label: content.label.clone(),
        has_image: content.thumbnail.is_some(),
        image: content
            .thumbnail
            .as_ref()
            .map(|picture| picture.image.clone())
            .unwrap_or_default(),
        failed: content.failed,
        selected: content.selected,
    }
}

fn result_picture(
    key: ResultImageKey,
    area: PreviewArea,
    pixels: &RgbaImage,
    pictures: &mut OnlinePictures<'_>,
) -> ShownPicture {
    let originals = &mut *pictures.originals;
    let original = || originals.image(key, || slint_image(pixels));
    pictures
        .previews
        .show(PictureId::Result(key), area, pixels, original)
}

fn image_key(state: &OnlineState, index: usize, image: ResultImage) -> ResultImageKey {
    ResultImageKey {
        generation: state.generation(),
        index,
        image,
    }
}

/// The text in place of an empty grid: the hint before the first search, how the latest search ended after
/// it, and nothing while a search runs (the status line says so then).
fn empty_text(state: &OnlineState, searching: bool) -> &str {
    if !state.results.is_empty() || searching {
        return "";
    }
    state.search_end_text.as_deref().unwrap_or(EMPTY_GRID_TEXT)
}

fn placeholder(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::TitleId => TITLE_ID_PLACEHOLDER,
        SearchMode::Keywords => KEYWORDS_PLACEHOLDER,
    }
}

fn position_of<T: PartialEq>(choices: &[T], value: T) -> i32 {
    choices
        .iter()
        .position(|choice| *choice == value)
        .map_or(NO_SELECTION, slint_int)
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
