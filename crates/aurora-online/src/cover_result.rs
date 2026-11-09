#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverResult {
    /// May span lines: an XboxUnity cover's rating is on a line of its own.
    pub label: String,
    pub image_url: String,
    pub thumbnail_url: Option<String>,
}

impl CoverResult {
    /// The label on one line, for a sentence that names the result.
    pub fn one_line_label(&self) -> String {
        self.label.lines().collect::<Vec<_>>().join(" ")
    }
}
