use std::fmt;

use super::BulkOutcome;
use crate::client::ConsoleError;
use crate::games::Game;

#[derive(Debug)]
pub struct BulkReport {
    /// Every game handled, in order; games after an abort are absent.
    pub entries: Vec<BulkEntry>,
    /// Set when reconnecting to the console failed and the run stopped there. The game being
    /// handled at that point has no entry, though its progress was already reported.
    pub aborted: Option<ConsoleError>,
}

impl BulkReport {
    pub fn summary(&self) -> BulkSummary {
        let mut summary = BulkSummary::default();
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
}

impl fmt::Display for BulkSummary {
    /// Reads like "3 covers uploaded, 2 already had one, 1 unreadable cover skipped,
    /// 1 without a cover online, 1 failed".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} uploaded, {} already had one, {} unreadable {} skipped, \
             {} without a cover online, {} failed",
            self.uploaded,
            cover_noun(self.uploaded),
            self.already_had_cover,
            self.unreadable,
            cover_noun(self.unreadable),
            self.no_cover_online,
            self.failed,
        )
    }
}

fn cover_noun(count: usize) -> &'static str {
    if count == 1 { "cover" } else { "covers" }
}
