use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::console::PresenceGate;

use aurora_asset::AssetKind;
use aurora_console::{
    AssetPresence, AuroraRevision, BulkError, BulkOptions, BulkReport, ConsoleError, FtpSettings,
    Game, GameIds, GameListing,
};
use aurora_online::CoverSource;

use super::{AfterDownload, AssetUpload, DownloadScope, KindDownload, TransferReport};

/// Work for the console's FTP server, done with `settings`.
#[derive(Debug)]
pub struct ConsoleRequest {
    pub settings: FtpSettings,
    pub job: ConsoleJob,
}

#[derive(Debug)]
pub enum ConsoleJob {
    Test,
    Refresh {
        generation: u64,
    },
    /// Which asset files one game has; `generation` is the refresh that listed the game. The worker skips
    /// the check when `gate` no longer wants it.
    Presence {
        generation: u64,
        ids: GameIds,
        gate: PresenceGate,
    },
    /// Downloads and decodes the kinds of `scope` in order; the first error ends the job.
    Download {
        ids: GameIds,
        scope: DownloadScope,
        after: AfterDownload,
    },
    /// Sends the files in order; the first error ends the job.
    Upload {
        ids: GameIds,
        uploads: Vec<AssetUpload>,
    },
    /// Replaces the kinds' files with the empty asset; the first error ends the job.
    Remove {
        ids: GameIds,
        kinds: Vec<AssetKind>,
    },
    /// Uploads a random online cover to each game that has none.
    Bulk(BulkRequest),
}

/// A bulk cover run: the games in the order they are handled, and the flag that stops it before the next game.
#[derive(Debug)]
pub struct BulkRequest {
    pub games: Vec<Game>,
    pub options: BulkOptions,
    pub source: CoverSource,
    pub stop: Arc<AtomicBool>,
}

/// Which kind of console job a progress report belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleJobKind {
    Test,
    Refresh,
    Presence,
    Download,
    Upload,
    Remove,
    Bulk,
}

impl ConsoleJob {
    pub fn kind(&self) -> ConsoleJobKind {
        match self {
            ConsoleJob::Test => ConsoleJobKind::Test,
            ConsoleJob::Refresh { .. } => ConsoleJobKind::Refresh,
            ConsoleJob::Presence { .. } => ConsoleJobKind::Presence,
            ConsoleJob::Download { .. } => ConsoleJobKind::Download,
            ConsoleJob::Upload { .. } => ConsoleJobKind::Upload,
            ConsoleJob::Remove { .. } => ConsoleJobKind::Remove,
            ConsoleJob::Bulk(_) => ConsoleJobKind::Bulk,
        }
    }
}

/// What a console job found, to be applied on the UI thread.
#[derive(Debug)]
pub enum ConsoleOutcome {
    Tested(Result<AuroraRevision, ConsoleError>),
    Refreshed {
        generation: u64,
        result: Result<GameListing, ConsoleError>,
    },
    Presence {
        generation: u64,
        ids: GameIds,
        result: Result<AssetPresence, ConsoleError>,
    },
    /// A presence check the gate no longer wanted; the console was not contacted.
    PresenceSkipped {
        generation: u64,
        ids: GameIds,
    },
    /// One item per kind that was tried, in order; an error is the last one.
    Downloaded {
        ids: GameIds,
        scope: DownloadScope,
        after: AfterDownload,
        results: Vec<KindDownload>,
    },
    Uploaded(TransferReport),
    Removed(TransferReport),
    Bulk(Result<BulkReport, BulkError>),
}
