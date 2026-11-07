use crate::console::PresenceGate;

use aurora_console::{
    AssetPresence, AuroraRevision, ConsoleError, FtpSettings, GameIds, GameListing,
};

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
}

/// Which kind of console job a progress report belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleJobKind {
    Test,
    Refresh,
    Presence,
}

impl ConsoleJob {
    pub fn kind(&self) -> ConsoleJobKind {
        match self {
            ConsoleJob::Test => ConsoleJobKind::Test,
            ConsoleJob::Refresh { .. } => ConsoleJobKind::Refresh,
            ConsoleJob::Presence { .. } => ConsoleJobKind::Presence,
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
}
