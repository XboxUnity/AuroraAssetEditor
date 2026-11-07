use super::BulkFailure;
use crate::client::ConsoleError;

#[derive(Debug)]
pub enum BulkOutcome {
    Uploaded,
    AlreadyHasCover,
    /// The console's cover file can't be parsed, so it is left alone.
    ExistingCoverUnreadable(ConsoleError),
    NoCoverFound,
    Failed(BulkFailure),
}
