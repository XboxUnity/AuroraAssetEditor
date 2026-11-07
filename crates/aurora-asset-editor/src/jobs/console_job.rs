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
    /// Which asset files one game has; `generation` is the refresh that listed the game.
    Presence {
        generation: u64,
        ids: GameIds,
    },
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
}
