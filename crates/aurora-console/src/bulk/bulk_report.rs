use super::{BulkOutcome, BulkStop};
use crate::games::Game;

#[derive(Debug)]
pub struct BulkReport {
    /// Every game handled, in order; games after a stop are absent.
    pub entries: Vec<BulkEntry>,
    /// How many games the run was given.
    pub total: usize,
    /// Set when the run ended early. The game being reached at that point has no entry, though
    /// its progress was already reported.
    pub stopped: Option<BulkStop>,
}

impl BulkReport {
    pub fn summary(&self) -> BulkSummary {
        let mut summary = BulkSummary {
            not_handled: self.total.saturating_sub(self.entries.len()),
            ..BulkSummary::default()
        };
        for entry in &self.entries {
            match entry.outcome {
                BulkOutcome::Uploaded => summary.uploaded += 1,
                BulkOutcome::AlreadyHasCover => summary.already_had_cover += 1,
                BulkOutcome::ExistingCoverUnreadable(_) => summary.unreadable += 1,
                BulkOutcome::NoCoverFound => summary.no_cover_online += 1,
                BulkOutcome::Failed(_) => summary.failed += 1,
            }
        }
        summary
    }
}

#[derive(Debug)]
pub struct BulkEntry {
    pub game: Game,
    pub outcome: BulkOutcome,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkSummary {
    pub uploaded: usize,
    pub already_had_cover: usize,
    pub unreadable: usize,
    pub no_cover_online: usize,
    pub failed: usize,
    /// Games the run stopped before.
    pub not_handled: usize,
}
