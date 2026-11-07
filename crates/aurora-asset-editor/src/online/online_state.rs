use aurora_online::CoverSource;

use super::{ResultTile, SearchMode};

/// The Online page's state: the search box, and the results of the latest search.
#[derive(Debug)]
pub struct OnlineState {
    pub source: CoverSource,
    pub mode: SearchMode,
    /// Each mode keeps its own text, as the old app's two boxes did.
    pub title_id_text: String,
    pub keywords_text: String,
    pub results: Vec<ResultTile>,
    pub selected: Option<usize>,
    /// What the latest search looked for, as shown in the status line.
    pub last_query: Option<String>,
    /// The search whose results are current; answers of an earlier search, and the images of its results,
    /// are stale.
    generation: u64,
    /// The source the results came from; the page's source can change afterwards.
    searched_source: CoverSource,
}

impl OnlineState {
    pub fn new() -> OnlineState {
        OnlineState {
            source: CoverSource::XboxUnity,
            mode: SearchMode::TitleId,
            title_id_text: String::new(),
            keywords_text: String::new(),
            results: Vec::new(),
            selected: None,
            last_query: None,
            generation: 0,
            searched_source: CoverSource::XboxUnity,
        }
    }

    /// The text of the box the page shows.
    pub fn search_text(&self) -> &str {
        match self.mode {
            SearchMode::TitleId => &self.title_id_text,
            SearchMode::Keywords => &self.keywords_text,
        }
    }

    pub fn selected_tile(&self) -> Option<&ResultTile> {
        self.results.get(self.selected?)
    }

    /// Empties the grid for a new search and returns its generation.
    pub(crate) fn start_search(&mut self, query_text: &str) -> u64 {
        self.generation += 1;
        self.results.clear();
        self.selected = None;
        self.last_query = Some(query_text.to_owned());
        self.searched_source = self.source;
        self.generation
    }

    pub fn searched_source(&self) -> CoverSource {
        self.searched_source
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

impl Default for OnlineState {
    fn default() -> Self {
        OnlineState::new()
    }
}
