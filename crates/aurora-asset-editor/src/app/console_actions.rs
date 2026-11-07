use std::error::Error;

use aurora_console::{
    AssetPresence, AuroraRevision, ConsoleError, ConsoleStatus, FTP_SETTINGS_FILE, FtpSettings,
    GameFilter, GameIds, GameListing,
};

use crate::console::{ConsoleLink, ConsoleState, GameColumn, PresenceCell, is_connection_failure};
use crate::dialogs::{ConnectionForm, Modal};
use crate::jobs::{ConsoleJob, ConsoleJobKind, ConsoleOutcome, ConsoleRequest, Job, JobPriority};
use crate::logging::error_chain;
use crate::status::{ConsoleActivity, Severity, StatusLine};

use super::file_actions::BUSY_TEXT;
use super::{App, CurrentGame};

pub const NO_CONSOLE_TEXT: &str = "No console connection is set up. Use Edit connection… first.";
pub const REFRESHING_TEXT: &str = "Refreshing the game list…";
pub const CONNECTION_FAILED_TITLE: &str = "Connection failed";
pub const TEMP_FILE_NOTE: &str = "temp file left behind (see error.log)";
pub(super) const CONNECTION_FAILED_PREFIX: &str = "FTP Connection could not be established";
const CONNECTION_TEST_FAILED: &str = "The connection test failed";
const NOT_AURORA_TEST_TEXT: &str = "Connection test Failed...";
const CONNECTION_SAVE_FAILED: &str = "Could not save the connection settings";
const REFRESH_FAILED: &str = "Could not refresh the game list";
const TEMP_CLEANUP_FAILED: &str = "Could not remove the temporary copy of Content.db";
const SUPERSEDED_REFRESH_FAILED: &str = "A refresh that was replaced failed";
const PRESENCE_UNREACHABLE: &str = "The console could not be reached to check the assets";
const PRESENCE_FAILED: &str = "Could not check the assets on the console";
const STATUS_SEPARATOR: &str = " · ";
const LAST_REFRESH_FORMAT: &str = "%H:%M";

impl App {
    pub fn console(&self) -> &ConsoleState {
        &self.console
    }

    /// Opens the connection dialog.
    pub fn edit_connection(&mut self) {
        if self.modals.connection_form_mut().is_some() {
            return;
        }
        let form = ConnectionForm::new(
            self.console.settings.as_ref(),
            &self.environment.subnet_prefix,
        );
        self.modals.push(Modal::Connection { form });
    }

    /// The dialog's Test Connection: the typed settings become the session's settings, saved or not.
    pub fn test_connection(&mut self) {
        if self.refuse_while_console_busy() {
            self.set_test_result(Severity::Info, BUSY_TEXT);
            return;
        }
        let Some(form) = self.modals.connection_form_mut() else {
            return;
        };
        form.test_status = None;
        let settings = form.to_settings();
        self.console.use_settings(settings.clone());
        self.active_work.console = Some(ConsoleActivity::Testing);
        self.submit_console_job(settings, ConsoleJob::Test, JobPriority::User);
    }

    /// The dialog's Save: writes `ftp.json`, makes the typed settings the session's and closes the dialog.
    pub fn save_connection(&mut self) {
        let Some(form) = self.modals.connection_form_mut() else {
            return;
        };
        let settings = form.to_settings();
        let path = self.environment.config_dir.join(FTP_SETTINGS_FILE);
        match settings.save(&path) {
            Ok(()) => {
                self.console.use_settings(settings);
                self.modals.remove_connection_form();
            }
            Err(error) => {
                let text = format!("{CONNECTION_SAVE_FAILED}: {}", error_sentence(&error));
                self.report_console_error(CONNECTION_SAVE_FAILED, &error);
                if let Some(form) = self.modals.connection_form_mut() {
                    form.test_status = Some((Severity::Error, text));
                }
            }
        }
    }

    /// Clears the table and lists the console's games again.
    pub fn refresh_games(&mut self) {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        self.console.table.clear();
        let generation = self.console.start_refresh();
        self.active_work.console = Some(ConsoleActivity::Refreshing);
        self.status = StatusLine::info(REFRESHING_TEXT);
        self.submit_console_job(
            settings,
            ConsoleJob::Refresh { generation },
            JobPriority::User,
        );
    }

    pub fn set_game_filter(&mut self, title: &str, title_id: &str) {
        self.console.table.set_filter(GameFilter {
            title: title.to_owned(),
            title_id: title_id.to_owned(),
        });
    }

    pub fn sort_games(&mut self, column: GameColumn) {
        self.console.table.sort_by(column);
    }

    /// Selecting a row makes its game the current game.
    pub fn select_game(&mut self, visible_row: usize) {
        let Some(game) = self.console.table.select_visible_row(visible_row) else {
            return;
        };
        let game = CurrentGame {
            title: game.title_name.clone(),
            ids: game.ids.clone(),
        };
        self.set_current_game(Some(game));
    }

    pub(super) fn apply_console_outcome(&mut self, outcome: ConsoleOutcome) {
        match outcome {
            ConsoleOutcome::Tested(result) => self.apply_tested(result),
            ConsoleOutcome::Refreshed { generation, result } => {
                self.apply_refreshed(generation, result);
            }
            ConsoleOutcome::Presence {
                generation,
                ids,
                result,
            } => self.apply_presence(generation, &ids, result),
            ConsoleOutcome::PresenceSkipped { generation, ids } => {
                self.apply_presence_skipped(generation, &ids);
            }
            ConsoleOutcome::Downloaded {
                ids,
                after,
                results,
            } => {
                self.apply_downloaded(&ids, after, results);
            }
            ConsoleOutcome::Uploaded(report) => self.apply_uploaded(report),
            ConsoleOutcome::Removed(report) => self.apply_removed(report),
            ConsoleOutcome::Bulk(result) => self.apply_bulk(result),
        }
    }

    /// Shows what the client reports while a job the user started runs. Presence checks report the same
    /// statuses in the background; they are not progress of anything the user is waiting for. A bulk run
    /// shows its own progress, which the client's reports would replace.
    pub(super) fn apply_console_status(&mut self, kind: ConsoleJobKind, status: &ConsoleStatus) {
        if kind == ConsoleJobKind::Presence {
            return;
        }
        let text = status.to_string();
        if let ConsoleStatus::AuroraFound { revision } = status {
            self.console.link = connected_link(revision);
        }
        if kind == ConsoleJobKind::Bulk {
            return;
        }
        if kind == ConsoleJobKind::Test
            && let Some(form) = self.modals.connection_form_mut()
        {
            form.test_status = Some((Severity::Info, text.clone()));
        }
        self.status.progress = Some(text);
    }

    fn apply_tested(&mut self, result: Result<AuroraRevision, ConsoleError>) {
        self.active_work.console = None;
        match result {
            Ok(revision) => {
                let text = format!(
                    "Connection to Aurora revision {} Established...",
                    revision.as_str()
                );
                self.console.link = connected_link(&revision);
                self.set_test_result(Severity::Info, &text);
                self.status = StatusLine::info(text);
            }
            Err(error) => {
                let text = test_failure_text(&error);
                let log_failure = self.record_error(CONNECTION_TEST_FAILED, &error);
                self.console.link = ConsoleLink::Failed;
                self.set_test_result(Severity::Error, &text);
                self.set_error_status(text, log_failure);
            }
        }
    }

    fn set_test_result(&mut self, severity: Severity, text: &str) {
        if let Some(form) = self.modals.connection_form_mut() {
            form.test_status = Some((severity, text.to_owned()));
        }
    }

    fn apply_refreshed(&mut self, generation: u64, result: Result<GameListing, ConsoleError>) {
        if generation != self.console.refresh_generation() {
            if let Err(error) = &result {
                self.log_superseded(SUPERSEDED_REFRESH_FAILED, error);
            }
            return;
        }
        self.active_work.console = None;
        match result {
            Ok(listing) => self.apply_listing(listing),
            Err(error) => self.apply_refresh_failure(&error),
        }
    }

    fn apply_listing(&mut self, listing: GameListing) {
        let count = listing.games.len();
        let refreshed_at = self.environment.clock.now();
        self.console.last_refresh = Some(refreshed_at);
        let mut text = format!(
            "Connected{STATUS_SEPARATOR}{count} games{STATUS_SEPARATOR}last refresh {}",
            refreshed_at.format(LAST_REFRESH_FORMAT)
        );
        let mut log_failure = None;
        if let Some(cleanup_error) = &listing.cleanup_error {
            log_failure = self.record_error(TEMP_CLEANUP_FAILED, cleanup_error);
            text.push_str(&format!("{STATUS_SEPARATOR}{TEMP_FILE_NOTE}"));
        }
        match log_failure {
            Some(_) => self.set_error_status(text, log_failure),
            None => self.status = StatusLine::info(text),
        }
        self.console.table.set_games(listing.games);
        self.queue_presence_checks();
    }

    fn apply_refresh_failure(&mut self, error: &ConsoleError) {
        if !is_connection_failure(error) {
            self.report_console_error(REFRESH_FAILED, error);
            return;
        }
        self.show_connection_failed(error);
    }

    /// The link is down: the status line and the "Connection failed" dialog say so.
    pub(super) fn show_connection_failed(&mut self, error: &ConsoleError) {
        self.console.link = ConsoleLink::Failed;
        let text = format!("{CONNECTION_FAILED_PREFIX}: {}", error_sentence(error));
        self.status = StatusLine::error(text.clone());
        self.show_error(CONNECTION_FAILED_TITLE, text, error);
    }

    /// One check per game, the games on screen first, behind everything the user starts.
    fn queue_presence_checks(&mut self) {
        let Some(settings) = self.console.settings.clone() else {
            return;
        };
        let generation = self.console.refresh_generation();
        for ids in self.console.table.ids_in_presence_order() {
            let job = ConsoleJob::Presence {
                generation,
                ids,
                gate: self.console.presence_gate().clone(),
            };
            self.submit_console_job(settings.clone(), job, JobPriority::Background);
        }
    }

    /// One check of a game whose assets just changed, ahead of the background checks still waiting.
    pub(super) fn queue_presence_check(&mut self, ids: &GameIds) {
        let Some(settings) = self.console.settings.clone() else {
            return;
        };
        if !self.console.table.contains(ids) {
            return;
        }
        let job = ConsoleJob::Presence {
            generation: self.console.refresh_generation(),
            ids: ids.clone(),
            gate: self.console.presence_gate().clone(),
        };
        self.submit_console_job(settings, job, JobPriority::User);
    }

    fn apply_presence(
        &mut self,
        generation: u64,
        ids: &GameIds,
        result: Result<AssetPresence, ConsoleError>,
    ) {
        if generation != self.console.refresh_generation() {
            if let Err(error) = &result {
                self.log_superseded(PRESENCE_FAILED, error);
            }
            return;
        }
        let cell = match result {
            Ok(presence) => PresenceCell::Known(presence),
            Err(error) if is_connection_failure(&error) => {
                self.console.link = ConsoleLink::Failed;
                self.report_console_error(PRESENCE_UNREACHABLE, &error);
                PresenceCell::Failed
            }
            Err(error) => {
                let context = format!("{PRESENCE_FAILED} for {}", ids.folder_name());
                if let Some(log_failure) = self.record_error(&context, &error) {
                    self.status = StatusLine::error(log_failure);
                }
                PresenceCell::Failed
            }
        };
        self.console.table.set_presence(ids, cell);
    }

    /// The worker skips the rest of a refresh's checks once one has found the console unreachable.
    fn apply_presence_skipped(&mut self, generation: u64, ids: &GameIds) {
        if generation == self.console.refresh_generation() {
            self.console.table.set_presence(ids, PresenceCell::Failed);
        }
    }

    /// A result of a replaced job is not shown, but its failure is still written to `error.log`.
    fn log_superseded(&mut self, context: &str, error: &ConsoleError) {
        if let Some(log_failure) = self.record_error(context, error) {
            self.status = StatusLine::error(log_failure);
        }
    }

    /// Logs the error and shows it, with its causes, in the status line.
    pub(super) fn report_console_error(&mut self, context: &str, error: &(dyn Error + 'static)) {
        let log_failure = self.record_error(context, error);
        self.set_error_status(format!("{context}: {}", error_sentence(error)), log_failure);
    }

    /// Like `report_console_error`, with `note` (what the failed job had done before) after the error.
    pub(super) fn report_console_error_noting(
        &mut self,
        context: &str,
        error: &(dyn Error + 'static),
        note: &str,
    ) {
        let log_failure = self.record_error(context, error);
        let text = format!("{context}: {}. {note}", error_sentence(error));
        self.set_error_status(text, log_failure);
    }

    pub(super) fn refuse_while_console_busy(&mut self) -> bool {
        if self.active_work.console.is_none() {
            return false;
        }
        self.status = StatusLine::info(BUSY_TEXT);
        true
    }

    pub(super) fn submit_console_job(
        &mut self,
        settings: FtpSettings,
        job: ConsoleJob,
        priority: JobPriority,
    ) {
        let request = ConsoleRequest { settings, job };
        self.environment
            .jobs
            .submit(Job::Console(request), priority);
    }
}

/// The old test said "Connection test Failed..." when the console did not answer like Aurora, and
/// "Error: {message}" for everything else.
fn test_failure_text(error: &ConsoleError) -> String {
    match error {
        ConsoleError::NotAurora { .. } => {
            format!("{NOT_AURORA_TEST_TEXT}: {}", error_sentence(error))
        }
        _ => format!("Error: {}", error_sentence(error)),
    }
}

fn connected_link(revision: &AuroraRevision) -> ConsoleLink {
    ConsoleLink::Connected {
        revision: revision.as_str().to_owned(),
    }
}

/// The error and its causes in one line, the way a user needs them to tell what went wrong.
pub(super) fn error_sentence(error: &(dyn Error + 'static)) -> String {
    error_chain(error).join(": ")
}
