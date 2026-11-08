use std::error::Error;

use aurora_asset::{AssetSlot, RgbaImage};
use aurora_online::{CoverQuery, CoverResult, CoverSource, OnlineError, QueryError, TitleId};

use crate::files::image_export_filters;
use crate::jobs::{
    FetchError, FetchedImage, ImageSource, ImportJob, ImportRequest, ImportSlot, Job, JobPriority,
    OnlineJob, OnlineOutcome, query_text,
};
use crate::online::{OnlineState, ResultTile, SearchMode, TileImage};
use crate::platform::SaveRequest;
use crate::status::StatusLine;
use crate::text::filter_hex_input;

use super::file_actions::BUSY_TEXT;
use super::{App, PendingImportStatus};

pub const ENTER_TITLE_ID_TEXT: &str = "Enter a TitleID to search.";
pub const ENTER_KEYWORDS_TEXT: &str = "Enter keywords to search.";
pub const IMAGE_FAILED_TEXT: &str = "Failed to load image";
pub const SEARCH_FAILED: &str = "Search failed";
pub const SAVE_RESULT_TITLE: &str = "Select where to save the cover";
pub const SAVE_RESULT_DEFAULT_NAME: &str = "cover.png";
pub const IMPORTING_COVER_TEXT: &str = "Importing cover…";
const DOWNLOADING_COVER_TEXT: &str = "Downloading cover…";
const SEARCH_TEXT_INVALID: &str = "The search text is not valid";
const SUPERSEDED_SEARCH_FAILED: &str = "A search that was replaced failed";
const SUPERSEDED_IMAGE_FAILED: &str = "An image of an earlier result failed";
const DEFAULT_FILTER: usize = 0;

impl App {
    pub fn online(&self) -> &OnlineState {
        &self.online
    }

    /// The Internet Archive only searches by TitleID, so choosing it switches the page to that mode.
    pub fn set_online_source(&mut self, source: CoverSource) {
        self.online.source = source;
        if source == CoverSource::InternetArchive {
            self.online.mode = SearchMode::TitleId;
        }
    }

    pub fn set_search_mode(&mut self, mode: SearchMode) {
        if mode == SearchMode::Keywords && self.online.source == CoverSource::InternetArchive {
            log::warn!("Keywords were chosen while the Internet Archive is the source");
            return;
        }
        self.online.mode = mode;
    }

    /// The search box was edited; TitleID mode keeps the old box's input rule.
    pub fn edit_search_text(&mut self, text: &str) {
        match self.online.mode {
            SearchMode::TitleId => self.online.title_id_text = filter_hex_input(text),
            SearchMode::Keywords => self.online.keywords_text = text.to_owned(),
        }
    }

    /// Clears the grid and asks the source for covers. Like the old app, a second search waits until the
    /// running one has answered.
    pub fn search_online(&mut self) {
        if self.locks().online_search {
            self.status = StatusLine::info(BUSY_TEXT);
            return;
        }
        let query = match self.search_query() {
            Ok(query) => query,
            Err(QueryError::Empty) => {
                self.status = StatusLine::info(empty_search_text(self.online.mode));
                return;
            }
            Err(error) => {
                self.report_error(SEARCH_TEXT_INVALID, &error);
                return;
            }
        };
        let source = self.online.source;
        let text = query_text(&query);
        let generation = self.online.start_search(&text);
        self.active_work.online_search = true;
        self.status = StatusLine::info(format!("Searching {} for {text}…", source_name(source)));
        let job = OnlineJob::Search {
            generation,
            source,
            query,
        };
        self.environment
            .jobs
            .submit(Job::Online(job), JobPriority::User);
    }

    /// Shows the result in the panel, downloading its full image unless it is there already. Selecting a
    /// tile whose download failed tries once more.
    pub fn select_result(&mut self, index: usize) {
        if index >= self.online.results.len() {
            return;
        }
        let already_selected = self.online.selected == Some(index);
        self.online.selected = Some(index);
        let tile = &self.online.results[index];
        match (&tile.full, already_selected) {
            (TileImage::Loaded { .. }, _) | (TileImage::Pending, true) => return,
            (TileImage::Pending | TileImage::Failed, _) => {}
        }
        if let Some(full) = reusable_thumbnail(tile) {
            self.online.results[index].full = full;
            return;
        }
        let url = tile.result.image_url.clone();
        self.online.results[index].full = TileImage::Pending;
        self.status = StatusLine::info(DOWNLOADING_COVER_TEXT);
        let job = OnlineJob::Full {
            generation: self.online.generation(),
            index,
            url,
            gate: self.online.search_gate().clone(),
        };
        self.submit_download(job, JobPriority::User);
    }

    /// Makes the selected result the cover, as it came: results are never resized (spec 6.3). The page
    /// stays on Online.
    pub fn use_result_as_cover(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        let Some((label, bytes)) =
            self.selected_full_image(|tile, bytes, _| (tile.result.label.clone(), bytes.to_vec()))
        else {
            log::warn!("Use as Cover was asked for while no result image is loaded");
            return;
        };
        let source = source_name(self.online.searched_source());
        let job = ImportJob {
            requests: vec![ImportRequest {
                source: ImageSource::Bytes { label, bytes },
                slot: ImportSlot::Slot(AssetSlot::Cover),
                resize: false,
            }],
            auto_resize: self.settings.auto_resize,
            compression: self.compression(),
        };
        self.active_work.importing += 1;
        self.environment
            .jobs
            .submit(Job::Import(job), JobPriority::User);
        self.pending_import_status = Some(PendingImportStatus {
            done: format!("Cover set from {source} result"),
            failed: format!("Could not set the cover from {source} result"),
        });
        self.status = StatusLine::info(IMPORTING_COVER_TEXT);
    }

    /// The panel's "Save image…": the decoded image into a file in the format the extension names.
    pub fn save_result_image(&mut self) {
        let Some(pixels) = self.selected_full_image(|_, _, pixels| pixels.clone()) else {
            log::warn!("Save image was asked for while no result image is loaded");
            return;
        };
        let request = SaveRequest {
            title: SAVE_RESULT_TITLE.to_owned(),
            filters: image_export_filters(),
            default_filter: DEFAULT_FILTER,
            default_name: Some(SAVE_RESULT_DEFAULT_NAME.to_owned()),
        };
        let Some(path) = self.ask_save_file(&request) else {
            return;
        };
        self.write_image_file(&pixels, &path);
    }

    pub(super) fn apply_online_outcome(&mut self, outcome: OnlineOutcome) {
        match outcome {
            OnlineOutcome::Found {
                generation,
                query_text,
                result,
            } => self.apply_found(generation, &query_text, result),
            OnlineOutcome::Thumbnail {
                generation,
                index,
                result,
            } => self.apply_thumbnail(generation, index, result),
            OnlineOutcome::Full {
                generation,
                index,
                result,
            } => self.apply_full(generation, index, result),
            OnlineOutcome::DownloadSkipped { generation } => {
                self.apply_skipped_download(generation)
            }
        }
    }

    fn search_query(&self) -> Result<CoverQuery, QueryError> {
        match self.online.mode {
            SearchMode::TitleId => {
                TitleId::parse(&self.online.title_id_text).map(CoverQuery::TitleId)
            }
            SearchMode::Keywords => CoverQuery::keywords(&self.online.keywords_text),
        }
    }

    fn apply_found(
        &mut self,
        generation: u64,
        query_text: &str,
        result: Result<Vec<CoverResult>, OnlineError>,
    ) {
        if !self.online.is_current(generation) {
            if let Err(error) = result {
                self.log_discarded(SUPERSEDED_SEARCH_FAILED, &error);
            }
            return;
        }
        self.active_work.online_search = false;
        match result {
            Ok(covers) => {
                self.status = StatusLine::info(results_text(covers.len(), query_text));
                self.online.results = covers.into_iter().map(ResultTile::new).collect();
                self.queue_thumbnails(generation);
            }
            Err(error) => self.report_error(SEARCH_FAILED, &error),
        }
        self.online.end_search(&self.status.text);
    }

    /// Thumbnails wait behind everything the user asks for.
    fn queue_thumbnails(&mut self, generation: u64) {
        let jobs: Vec<OnlineJob> = self
            .online
            .results
            .iter()
            .enumerate()
            .map(|(index, tile)| OnlineJob::Thumbnail {
                generation,
                index,
                url: tile.thumbnail_source_url().to_owned(),
                gate: self.online.search_gate().clone(),
            })
            .collect();
        for job in jobs {
            self.submit_download(job, JobPriority::Background);
        }
    }

    fn submit_download(&mut self, job: OnlineJob, priority: JobPriority) {
        self.active_work.online_downloads += 1;
        self.environment.jobs.submit(Job::Online(job), priority);
    }

    fn apply_thumbnail(
        &mut self,
        generation: u64,
        index: usize,
        result: Result<FetchedImage, FetchError>,
    ) {
        self.finish_download();
        match result {
            Ok(image) => {
                if let Some(tile) = self.current_tile_mut(generation, index) {
                    tile.thumbnail = loaded(image);
                }
            }
            Err(error) => {
                let context = match self.current_tile_mut(generation, index) {
                    Some(tile) => {
                        tile.thumbnail = TileImage::Failed;
                        format!("Could not load the thumbnail of {}", tile.result.label)
                    }
                    None => SUPERSEDED_IMAGE_FAILED.to_owned(),
                };
                self.log_discarded(&context, &error);
            }
        }
    }

    /// A full image counts only while its tile is still the selected one; a late one is dropped, not kept.
    fn apply_full(
        &mut self,
        generation: u64,
        index: usize,
        result: Result<FetchedImage, FetchError>,
    ) {
        self.finish_download();
        let selected = self.online.selected == Some(index);
        let tile = if selected {
            self.current_tile_mut(generation, index)
        } else {
            None
        };
        let Some(tile) = tile else {
            if let Err(error) = result {
                self.log_discarded(SUPERSEDED_IMAGE_FAILED, &error);
            }
            return;
        };
        match result {
            Ok(image) => {
                tile.full = loaded(image);
                let query = self.online.last_query.clone().unwrap_or_default();
                self.status = StatusLine::info(results_text(self.online.results.len(), &query));
            }
            Err(error) => {
                tile.full = TileImage::Failed;
                let context = format!("Could not load the image of {}", tile.result.label);
                let log_failure = self.record_error(&context, &error);
                self.set_error_status(IMAGE_FAILED_TEXT.to_owned(), log_failure);
            }
        }
    }

    /// The worker skips only the downloads of a replaced search, so a skip of the current one is a bug.
    fn apply_skipped_download(&mut self, generation: u64) {
        self.finish_download();
        if self.online.is_current(generation) {
            log::error!("A download of the current search {generation} was skipped as stale");
        }
    }

    pub(super) fn finish_download(&mut self) {
        self.active_work.online_downloads = self.active_work.online_downloads.saturating_sub(1);
    }

    /// The tile at `index`, when `generation` is still the current search.
    pub(super) fn current_tile_mut(
        &mut self,
        generation: u64,
        index: usize,
    ) -> Option<&mut ResultTile> {
        if !self.online.is_current(generation) {
            return None;
        }
        self.online.results.get_mut(index)
    }

    /// `read` applied to the selected tile and its loaded full image's file and pixels; `None` while the
    /// image is not loaded.
    fn selected_full_image<T>(
        &self,
        read: impl FnOnce(&ResultTile, &[u8], &RgbaImage) -> T,
    ) -> Option<T> {
        let tile = self.online.selected_tile()?;
        match &tile.full {
            TileImage::Loaded { pixels, bytes } => Some(read(tile, bytes, pixels)),
            TileImage::Pending | TileImage::Failed => None,
        }
    }

    /// Writes the error to `error.log` for a result that is not shown any more. When writing fails the
    /// status line says so.
    fn log_discarded(&mut self, context: &str, error: &(dyn Error + 'static)) {
        if let Some(log_failure) = self.record_error(context, error) {
            self.status = StatusLine::error(log_failure);
        }
    }
}

fn loaded(image: FetchedImage) -> TileImage {
    TileImage::Loaded {
        pixels: image.pixels,
        bytes: image.bytes,
    }
}

/// The thumbnail, when it is the full image: the same address, already downloaded.
fn reusable_thumbnail(tile: &ResultTile) -> Option<TileImage> {
    if tile.thumbnail_source_url() != tile.result.image_url {
        return None;
    }
    match &tile.thumbnail {
        TileImage::Loaded { .. } => Some(tile.thumbnail.clone()),
        TileImage::Pending | TileImage::Failed => None,
    }
}

fn source_name(source: CoverSource) -> &'static str {
    match source {
        CoverSource::XboxUnity => "XboxUnity",
        CoverSource::InternetArchive => "Internet Archive",
    }
}

fn empty_search_text(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::TitleId => ENTER_TITLE_ID_TEXT,
        SearchMode::Keywords => ENTER_KEYWORDS_TEXT,
    }
}

fn results_text(count: usize, query: &str) -> String {
    match count {
        1 => format!("1 result for {query}"),
        _ => format!("{count} results for {query}"),
    }
}
