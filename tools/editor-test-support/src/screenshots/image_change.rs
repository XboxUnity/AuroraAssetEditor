use super::pixel_comparison::PixelComparison;

/// What recording one image found out about it, compared with the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageChange {
    Unchanged,
    Changed(PixelComparison),
    /// The baseline has no such image, or there is no baseline.
    New,
    /// The baseline's image could not be used; the text says why.
    BaselineUnreadable(String),
    Error(String),
}

impl ImageChange {
    /// Only an error fails a run; the other outcomes are for the person reviewing the sheet.
    pub fn is_failure(&self) -> bool {
        matches!(self, ImageChange::Error(_))
    }
}
