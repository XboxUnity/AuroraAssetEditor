use aurora_asset::AssetKind;
use aurora_console::{ConsoleError, GameIds};

/// How an upload or a removal went for one game: the kinds that were sent, and the kind that failed, which
/// ended the transfer.
#[derive(Debug)]
pub struct TransferReport {
    pub ids: GameIds,
    pub done: Vec<AssetKind>,
    pub failure: Option<(AssetKind, ConsoleError)>,
}

impl TransferReport {
    pub fn new(ids: GameIds) -> TransferReport {
        TransferReport {
            ids,
            done: Vec::new(),
            failure: None,
        }
    }

    /// Notes how one kind went; `false` when it failed, which ends the transfer.
    pub fn record(&mut self, kind: AssetKind, result: Result<(), ConsoleError>) -> bool {
        match result {
            Ok(()) => {
                self.done.push(kind);
                true
            }
            Err(error) => {
                self.failure = Some((kind, error));
                false
            }
        }
    }
}
