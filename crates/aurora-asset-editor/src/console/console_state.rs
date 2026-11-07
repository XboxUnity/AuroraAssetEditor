use aurora_console::FtpSettings;
use chrono::{DateTime, Local};

use super::{ConsoleLink, GameTable, PresenceGate};

/// The console page's state: the connection settings of this session, what is known about the link, and
/// the game table.
#[derive(Debug)]
pub struct ConsoleState {
    pub settings: Option<FtpSettings>,
    pub link: ConsoleLink,
    pub table: GameTable,
    pub last_refresh: Option<DateTime<Local>>,
    presence_gate: PresenceGate,
}

impl ConsoleState {
    pub fn new(settings: Option<FtpSettings>) -> ConsoleState {
        let link = if settings.is_some() {
            ConsoleLink::Unknown
        } else {
            ConsoleLink::NotSetUp
        };
        ConsoleState {
            settings,
            link,
            table: GameTable::default(),
            last_refresh: None,
            presence_gate: PresenceGate::default(),
        }
    }

    /// The refresh whose results are current; results of an earlier one are stale.
    pub fn refresh_generation(&self) -> u64 {
        self.presence_gate.current()
    }

    pub(crate) fn presence_gate(&self) -> &PresenceGate {
        &self.presence_gate
    }

    pub(crate) fn start_refresh(&mut self) -> u64 {
        self.presence_gate.start_refresh()
    }

    /// Different settings say nothing about the link until they are tried.
    pub(crate) fn use_settings(&mut self, settings: FtpSettings) {
        if self.settings.as_ref() == Some(&settings) {
            return;
        }
        self.settings = Some(settings);
        self.link = ConsoleLink::Unknown;
    }
}
