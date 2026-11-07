use aurora_console::{HexId, IdError};

use crate::text::filter_hex_input;

/// The Save All dialog's text box. Edits are filtered like the old TitleID boxes, and OK needs all eight digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleIdForm {
    pub title_id: String,
}

impl TitleIdForm {
    pub fn new(prefill: &str) -> TitleIdForm {
        TitleIdForm {
            title_id: filter_hex_input(prefill),
        }
    }

    pub fn edit(&mut self, text: &str) {
        self.title_id = filter_hex_input(text);
    }

    pub fn is_valid(&self) -> bool {
        HexId::parse(&self.title_id).is_ok()
    }

    pub fn parse(&self) -> Result<HexId, IdError> {
        HexId::parse(&self.title_id)
    }
}
