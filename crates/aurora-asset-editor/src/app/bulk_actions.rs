use aurora_console::{
    AssetPresence, BulkEntry, BulkError, BulkOptions, BulkOutcome, BulkReport, BulkStop, Game,
};
use aurora_online::CoverSource;

use crate::console::{PresenceCell, is_connection_failure};
use crate::dialogs::{BulkForm, Modal};
use crate::jobs::{BulkRequest, ConsoleJob, JobPriority};
use crate::status::{ConsoleActivity, StatusLine};

use super::App;
use super::console_actions::{NO_CONSOLE_TEXT, error_sentence};

pub const BULK_TITLE: &str = "Bulk actions";
pub const BULK_FINISHED_TITLE: &str = "Bulk actions finished";
pub const NO_ASSETS_TEXT: &str = "No Assets listed";
const BULK_START_FAILED: &str = "Could not start the bulk covers";
const BULK_COVER_FAILED: &str = "Bulk covers could not set the cover of";
const BULK_COVER_UNREADABLE: &str =
    "Bulk covers skipped the cover on the console, which cannot be read, of";
const BULK_CONSOLE_GONE: &str = "Bulk covers stopped, the console cannot be reached again";

impl App {
    /// "Bulk actions…": asks for the options when the table lists games.
    pub fn open_bulk_actions(&mut self) {
        if self.modals.bulk_form_mut().is_some() {
            return;
        }
        let game_count = self.console.table.visible().len();
        if game_count == 0 {
            self.show_no_assets();
            return;
        }
        if self.console.settings.is_none() {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        }
        if self.refuse_while_console_busy() {
            return;
        }
        self.modals.push(Modal::Bulk {
            form: BulkForm::new(game_count),
        });
    }

    pub fn set_bulk_source(&mut self, source: CoverSource) {
        if let Some(form) = self.modals.bulk_form_mut() {
            form.source = source;
        }
    }

    pub fn set_bulk_replace(&mut self, replace_existing: bool) {
        if let Some(form) = self.modals.bulk_form_mut() {
            form.replace_existing = replace_existing;
        }
    }

    /// Ends the bulk run before the game it has not started yet.
    pub fn stop_bulk(&mut self) {
        self.console.request_bulk_stop();
    }

    /// Start of the bulk dialog: the games on screen, in the order they are shown.
    pub(super) fn start_bulk(&mut self, form: &BulkForm) {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        let games: Vec<Game> = self.console.table.visible().into_iter().cloned().collect();
        let request = BulkRequest {
            games,
            options: BulkOptions {
                replace_existing: form.replace_existing,
                compression: self.compression(),
            },
            source: form.source,
            stop: self.console.start_bulk_run(),
        };
        self.active_work.console = Some(ConsoleActivity::Bulk);
        self.submit_console_job(settings, ConsoleJob::Bulk(request), JobPriority::User);
    }

    pub(super) fn apply_bulk_progress(&mut self, position: usize, total: usize, title: &str) {
        self.status.progress = Some(format!("Bulk covers: {position} of {total} · {title}"));
    }

    pub(super) fn apply_bulk(&mut self, result: Result<BulkReport, BulkError>) {
        self.active_work.console = None;
        self.console.finish_bulk_run();
        match result {
            Ok(report) => self.show_bulk_report(&report),
            Err(BulkError::NoGames) => self.show_no_assets(),
            Err(BulkError::Connection(error)) if is_connection_failure(&error) => {
                self.show_connection_failed(&error);
            }
            Err(BulkError::Connection(error)) => {
                self.report_console_error(BULK_START_FAILED, &error);
            }
        }
    }

    fn show_no_assets(&mut self) {
        self.modals.push(Modal::Message {
            title: BULK_TITLE.to_owned(),
            text: NO_ASSETS_TEXT.to_owned(),
        });
    }

    fn show_bulk_report(&mut self, report: &BulkReport) {
        let log_failure = self.log_bulk_problems(report);
        self.mark_uploaded_covers(report);
        let summary = report.summary().to_string();
        let mut text = summary.clone();
        if let Some(stop) = &report.stopped {
            text.push_str(&format!("\nStopped: {}", error_sentence(stop)));
        }
        self.set_error_status_if(summary, log_failure);
        self.modals.push(Modal::Message {
            title: BULK_FINISHED_TITLE.to_owned(),
            text,
        });
    }

    /// The summary as the status; an error when `error.log` could not be written.
    fn set_error_status_if(&mut self, summary: String, log_failure: Option<String>) {
        match log_failure {
            Some(_) => self.set_error_status(summary, log_failure),
            None => self.status = StatusLine::info(summary),
        }
    }

    /// Writes every game that failed or whose cover could not be read, and a console that went away, to
    /// `error.log`. Returns the first failure to write the log.
    fn log_bulk_problems(&self, report: &BulkReport) -> Option<String> {
        let mut log_failures = Vec::new();
        for BulkEntry { game, outcome } in &report.entries {
            let log_failure = match outcome {
                BulkOutcome::Failed(failure) => {
                    self.record_error(&bulk_context(BULK_COVER_FAILED, game), failure)
                }
                BulkOutcome::ExistingCoverUnreadable(error) => {
                    self.record_error(&bulk_context(BULK_COVER_UNREADABLE, game), error)
                }
                BulkOutcome::Uploaded
                | BulkOutcome::AlreadyHasCover
                | BulkOutcome::NoCoverFound => None,
            };
            log_failures.extend(log_failure);
        }
        if let Some(stop @ BulkStop::ConsoleGone(_)) = &report.stopped {
            log_failures.extend(self.record_error(BULK_CONSOLE_GONE, stop));
        }
        log_failures.into_iter().next()
    }

    /// The console now has a cover for each uploaded game. A cell that does not say what else the game has
    /// is filled in by a check instead.
    fn mark_uploaded_covers(&mut self, report: &BulkReport) {
        let uploaded: Vec<&Game> = report
            .entries
            .iter()
            .filter(|entry| matches!(entry.outcome, BulkOutcome::Uploaded))
            .map(|entry| &entry.game)
            .collect();
        if uploaded.is_empty() {
            return;
        }
        self.console.presence_gate().mark_reachable();
        for game in uploaded {
            match self.console.table.presence(&game.ids) {
                PresenceCell::Known(presence) => self.console.table.set_presence(
                    &game.ids,
                    PresenceCell::Known(AssetPresence {
                        cover: true,
                        ..presence
                    }),
                ),
                PresenceCell::Unknown | PresenceCell::Failed => {
                    self.queue_presence_check(&game.ids);
                }
            }
        }
    }
}

fn bulk_context(what: &str, game: &Game) -> String {
    format!("{what} {} ({})", game.title_name, game.ids.title_id)
}
