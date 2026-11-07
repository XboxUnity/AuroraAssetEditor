use super::{BulkEntry, BulkOutcome, BulkSummary};
use crate::client::ConsoleError;

#[derive(Debug)]
pub struct BulkReport {
    /// Every game handled, in order; games after an abort are absent.
    pub entries: Vec<BulkEntry>,
    /// Set when reconnecting to the console failed and the run stopped there.
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
