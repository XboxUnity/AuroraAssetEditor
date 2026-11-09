use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use aurora_console::FtpSettings;
use chrono::{DateTime, Local};

use super::{ConsoleLink, GameTable, PresenceGate, RefreshOrigin};

/// The console page's state: the connection settings of this session, what is known about the link, and
/// the game table.
#[derive(Debug)]
pub struct ConsoleState {
    pub settings: Option<FtpSettings>,
    pub link: ConsoleLink,
    pub table: GameTable,
    pub last_refresh: Option<DateTime<Local>>,
    presence_gate: PresenceGate,
    /// The generation of the refresh that is running, if one is.
    running_refresh: Option<u64>,
    refresh_origin: RefreshOrigin,
    bulk_stop: Option<Arc<AtomicBool>>,
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
            running_refresh: None,
            refresh_origin: RefreshOrigin::default(),
            bulk_stop: None,
        }
    }

    /// The refresh whose results are current; results of an earlier one are stale.
    pub fn refresh_generation(&self) -> u64 {
        self.presence_gate.current()
    }

    pub fn presence_gate(&self) -> &PresenceGate {
        &self.presence_gate
    }

    pub(crate) fn start_refresh(&mut self, origin: RefreshOrigin) -> u64 {
        let generation = self.presence_gate.start_refresh();
        self.running_refresh = Some(generation);
        self.refresh_origin = origin;
        generation
    }

    pub fn refresh_origin(&self) -> RefreshOrigin {
        self.refresh_origin
    }

    pub fn is_refreshing(&self) -> bool {
        self.running_refresh.is_some()
    }

    pub(crate) fn finish_refresh(&mut self) {
        self.running_refresh = None;
    }

    /// A refresh runs for settings that were replaced since it started.
    pub(crate) fn running_refresh_is_stale(&self) -> bool {
        self.running_refresh
            .is_some_and(|generation| generation != self.refresh_generation())
    }

    /// Makes the flag that stops the bulk run that is about to start.
    pub(crate) fn start_bulk_run(&mut self) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        self.bulk_stop = Some(Arc::clone(&stop));
        stop
    }

    pub(crate) fn request_bulk_stop(&self) {
        if let Some(stop) = &self.bulk_stop {
            stop.store(true, Ordering::SeqCst);
        }
    }

    pub(crate) fn finish_bulk_run(&mut self) {
        self.bulk_stop = None;
    }

    /// Different settings say nothing about the link until they are tried, and the games listed so far
    /// belong to the old console: the table empties, and the refresh and presence checks still running for
    /// the old settings become stale.
    pub(crate) fn use_settings(&mut self, settings: FtpSettings) {
        if self.settings.as_ref() == Some(&settings) {
            return;
        }
        self.settings = Some(settings);
        self.link = ConsoleLink::Unknown;
        self.table.clear();
        self.last_refresh = None;
        self.presence_gate.start_refresh();
    }
}
