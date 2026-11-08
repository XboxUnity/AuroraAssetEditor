use aurora_online::CoverSource;

use super::{ResultTile, SearchGate, SearchMode};

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
    /// What the status line said when the latest search ended; the grid shows it when nothing was found.
    pub search_end_text: Option<String>,
    /// The search whose results are current; answers of an earlier search, and the images of its results,
    /// are stale.
    search_gate: SearchGate,
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
            search_end_text: None,
            search_gate: SearchGate::default(),
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
        let generation = self.search_gate.start_search();
        self.results.clear();
        self.selected = None;
        self.last_query = Some(query_text.to_owned());
        self.search_end_text = None;
        self.searched_source = self.source;
        generation
    }

    /// Records how the current search ended, as the status line says it.
    pub(crate) fn end_search(&mut self, status_text: &str) {
        self.search_end_text = Some(status_text.to_owned());
    }

    pub fn searched_source(&self) -> CoverSource {
        self.searched_source
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        !self.search_gate.is_superseded(generation)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.search_gate.current()
    }

    pub(crate) fn search_gate(&self) -> &SearchGate {
        &self.search_gate
    }
}

impl Default for OnlineState {
    fn default() -> Self {
        OnlineState::new()
    }
}
