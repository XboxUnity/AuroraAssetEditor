use aurora_online::CoverSource;

/// The bulk actions dialog: where the covers come from and whether covers on the console are replaced.
/// `game_count` is how many games the run is given, for the dialog's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkForm {
    pub source: CoverSource,
    pub replace_existing: bool,
    pub game_count: usize,
}

impl BulkForm {
    pub fn new(game_count: usize) -> BulkForm {
        BulkForm {
            source: CoverSource::XboxUnity,
            replace_existing: false,
            game_count,
        }
    }
}
