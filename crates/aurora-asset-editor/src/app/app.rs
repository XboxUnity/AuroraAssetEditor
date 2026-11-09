use std::collections::VecDeque;
use std::error::Error;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetKind, ScreenshotNumber};

use crate::about::BuildInfo;
use crate::console::ConsoleState;
use crate::dialogs::{
    ConfirmedAction, Modal, ModalAnswer, ModalField, ModalQueue, UnofferedChoice,
};
use crate::editor::EditorPages;
use crate::jobs::{JobOutcome, Progress};
use crate::logging::error_chain;
use crate::online::OnlineState;
use crate::platform::{DialogError, OpenRequest, SaveRequest};
use crate::settings::AppSettings;
use crate::status::{ActiveWork, Locks, STATUS_SEPARATOR, StatusLine, Toasts};

use super::{
    AppEnvironment, CurrentGame, Menu, Page, PendingImportStatus, PendingReplacement,
    StartupProblem,
};

pub const LOG_WRITE_FAILED: &str = "Could not write error.log";
pub const DIALOG_FAILED_TITLE: &str = "Could not open the file dialog";

/// The app's state and every action the window offers. It holds no UI: the view reads it and calls its
/// methods, and work that may take a while goes to the job runner.
pub struct App {
    pub(super) environment: AppEnvironment,
    pub(super) settings: AppSettings,
    /// `settings.json` could not be read at start and is left as it is until the user changes a setting.
    pub(super) keep_settings_file: bool,
    pub(super) page: Page,
    pub(super) open_menu: Option<Menu>,
    pub(super) pages: EditorPages,
    pub(super) current_game: Option<CurrentGame>,
    pub(super) console: ConsoleState,
    pub(super) online: OnlineState,
    pub(super) status: StatusLine,
    pub(super) toasts: Toasts,
    pub(super) active_work: ActiveWork,
    pub(super) modals: ModalQueue,
    pub(super) selected_screenshot: ScreenshotNumber,
    pub(super) drop_hover: bool,
    /// Set while an import with its own status text is queued; the editor is locked meanwhile, so no
    /// other import can end first.
    pub(super) pending_import_status: Option<PendingImportStatus>,
    /// The About dialog's "Copy build info" worked; it says so until the dialog closes.
    pub(super) about_copied: bool,
    pub(super) about_notices_expanded: bool,
    /// The change count of each page the running upload sends, so a page that reached the console is saved.
    pub(super) upload_change_counts: Vec<(AssetKind, u64)>,
    /// Work that replaces pages with unsaved changes, oldest first, each waiting for its Discard question.
    pub(super) pending_replacements: VecDeque<PendingReplacement>,
    /// The user discarded the unsaved changes to close the window.
    pub(super) close_approved: bool,
}

impl App {
    pub fn new(mut environment: AppEnvironment) -> App {
        let startup_problems = std::mem::take(&mut environment.startup_problems);
        let settings = std::mem::take(&mut environment.settings);
        let keep_settings_file = environment.keep_settings_file;
        let console = ConsoleState::new(environment.ftp_settings.take());
        let mut app = App {
            environment,
            settings,
            keep_settings_file,
            page: Page::Cover,
            open_menu: None,
            pages: EditorPages::new(),
            current_game: None,
            console,
            online: OnlineState::new(),
            status: StatusLine::ready(),
            toasts: Toasts::default(),
            active_work: ActiveWork::default(),
            modals: ModalQueue::new(),
            selected_screenshot: ScreenshotNumber::ALL[0],
            drop_hover: false,
            pending_import_status: None,
            about_copied: false,
            about_notices_expanded: false,
            upload_change_counts: Vec::new(),
            pending_replacements: VecDeque::new(),
            close_approved: false,
        };
        app.report_startup_problems(&startup_problems);
        app
    }

    pub fn config_dir(&self) -> &Path {
        &self.environment.config_dir
    }

    pub fn build_info(&self) -> &BuildInfo {
        &self.environment.build
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn show_page(&mut self, page: Page) {
        self.page = page;
    }

    pub fn pages(&self) -> &EditorPages {
        &self.pages
    }

    pub fn status(&self) -> &StatusLine {
        &self.status
    }

    pub fn locks(&self) -> Locks {
        self.active_work.locks()
    }

    pub fn modal(&self) -> Option<&Modal> {
        self.modals.current()
    }

    pub fn answer_modal(&mut self, answer: ModalAnswer) {
        self.apply_answer(answer);
        self.recheck_discard_question();
    }

    fn apply_answer(&mut self, answer: ModalAnswer) {
        if let Some(rejected) = self.unoffered_choice(answer) {
            self.report_error(
                "A dialog was answered with a button it does not have",
                &rejected,
            );
            return;
        }
        if self.form_not_ready(answer) {
            return;
        }
        // The upload waits for the console, which the dialog's own list refresh may hold: the dialog stays open.
        if answer == ModalAnswer::Accept
            && matches!(self.modals.current(), Some(Modal::UploadTarget { .. }))
            && self.refuse_console_transfer()
        {
            return;
        }
        if answer == ModalAnswer::Accept
            && matches!(self.modals.current(), Some(Modal::Connection { .. }))
        {
            self.save_connection();
            return;
        }
        let Some(modal) = self.modals.close_current() else {
            return;
        };
        match (modal, answer) {
            (Modal::Message { .. } | Modal::Connection { .. } | Modal::About, _) => {}
            (Modal::Bulk { form }, ModalAnswer::Accept) => self.start_bulk(&form),
            (Modal::Bulk { .. }, ModalAnswer::Cancel | ModalAnswer::Choice(_)) => {}
            (Modal::UploadTarget { form, .. }, ModalAnswer::Accept) => {
                self.save_all_to_console_for_form(&form);
            }
            (Modal::Confirm { action, .. }, ModalAnswer::Accept) => self.run_confirmed(action),
            (
                Modal::Confirm {
                    action: ConfirmedAction::DiscardChanges,
                    ..
                },
                ModalAnswer::Cancel | ModalAnswer::Choice(_),
            ) => self.keep_changes(),
            (
                Modal::UploadTarget { .. } | Modal::Confirm { .. },
                ModalAnswer::Cancel | ModalAnswer::Choice(_),
            ) => {}
            (Modal::TitleId { form, .. }, ModalAnswer::Accept) => self.save_all_for_form(&form),
            (Modal::TitleId { .. }, ModalAnswer::Cancel | ModalAnswer::Choice(_)) => {}
            (Modal::Choice { pending, .. }, ModalAnswer::Choice(choice)) => {
                self.answer_choice(pending, Some(choice));
            }
            (Modal::Choice { pending, .. }, ModalAnswer::Accept | ModalAnswer::Cancel) => {
                self.answer_choice(pending, None);
            }
        }
    }

    /// OK on a form does nothing until its boxes hold valid IDs; the dialog stays open.
    fn form_not_ready(&self, answer: ModalAnswer) -> bool {
        if answer != ModalAnswer::Accept {
            return false;
        }
        match self.modals.current() {
            Some(Modal::TitleId { form, .. }) => !form.is_valid(),
            Some(Modal::UploadTarget { form, .. }) => !form.is_valid(),
            _ => false,
        }
    }

    /// The answer's choice when the dialog on screen does not offer it.
    fn unoffered_choice(&self, answer: ModalAnswer) -> Option<UnofferedChoice> {
        let ModalAnswer::Choice(id) = answer else {
            return None;
        };
        let offered = match self.modals.current()? {
            Modal::Choice { choices, .. } => choices.iter().any(|(offered, _)| *offered == id),
            Modal::Message { .. }
            | Modal::TitleId { .. }
            | Modal::UploadTarget { .. }
            | Modal::Confirm { .. }
            | Modal::Connection { .. }
            | Modal::Bulk { .. }
            | Modal::About => false,
        };
        if offered {
            None
        } else {
            Some(UnofferedChoice(id))
        }
    }

    pub fn edit_modal_field(&mut self, field: ModalField, text: &str) {
        match (self.modals.current_mut(), field) {
            (Some(Modal::TitleId { form, .. }), ModalField::TitleId) => form.edit(text),
            (Some(Modal::UploadTarget { form, .. }), ModalField::UploadTitleId) => {
                form.edit_title_id(text);
            }
            (Some(Modal::UploadTarget { form, .. }), ModalField::UploadDatabaseId) => {
                form.edit_database_id(text);
            }
            (Some(Modal::Connection { form }), ModalField::Host) => form.host = text.to_owned(),
            (Some(Modal::Connection { form }), ModalField::Port) => form.port = text.to_owned(),
            (Some(Modal::Connection { form }), ModalField::Username) => {
                form.username = text.to_owned();
            }
            (Some(Modal::Connection { form }), ModalField::Password) => {
                form.password = text.to_owned();
            }
            _ => log::warn!("{field:?} was edited while its dialog is not on screen"),
        }
    }

    pub fn on_job_outcome(&mut self, outcome: JobOutcome) {
        match outcome {
            JobOutcome::Loaded(loaded) => self.apply_loaded(loaded),
            JobOutcome::Imported(results) => self.apply_imported(results),
            JobOutcome::Console(outcome) => self.apply_console_outcome(outcome),
            JobOutcome::Online(outcome) => self.apply_online_outcome(outcome),
            JobOutcome::Progress(Progress::Console { kind, status }) => {
                self.apply_console_status(kind, &status);
            }
            JobOutcome::Progress(Progress::Bulk {
                position,
                total,
                title,
            }) => self.apply_bulk_progress(position, total, &title),
            JobOutcome::Failed { job, panic } => self.apply_failed_job(&job, &panic),
            JobOutcome::Preview(resized) => log::error!(
                "A resized preview of {:?} reached the app; only the view shows previews",
                resized.picture
            ),
        }
    }

    /// Logs the error and shows it in the status line.
    pub(super) fn report_error(&mut self, context: &str, error: &(dyn Error + 'static)) {
        let log_failure = self.record_error(context, error);
        self.set_error_status(format!("{context}: {error}"), log_failure);
    }

    /// Logs the error and shows `text` in a message dialog titled `title`.
    pub(super) fn show_error(&mut self, title: &str, text: String, error: &(dyn Error + 'static)) {
        self.show_error_logged_as(title, title, text, error);
    }

    /// Like `show_error`, with `log_context` in place of the title as the log entry's context.
    fn show_error_logged_as(
        &mut self,
        log_context: &str,
        title: &str,
        text: String,
        error: &(dyn Error + 'static),
    ) {
        // The dialog shows the error; a failure to log it shows nowhere else.
        if let Some(log_failure) = self.record_error(log_context, error) {
            self.report_failure(log_failure);
        }
        self.modals.push(Modal::Message {
            title: title.to_owned(),
            text,
        });
    }

    /// The picked files; none when the user cancelled or the dialog failed, which is shown.
    pub(super) fn ask_open_files(&mut self, request: &OpenRequest) -> Vec<PathBuf> {
        let answer = self.environment.dialogs.open_files(request);
        self.dialog_answer(&request.title, answer)
            .unwrap_or_default()
    }

    /// The picked file; `None` when the user cancelled or the dialog failed, which is shown.
    pub(super) fn ask_open_file(&mut self, request: &OpenRequest) -> Option<PathBuf> {
        let answer = self.environment.dialogs.open_file(request);
        self.dialog_answer(&request.title, answer).flatten()
    }

    /// The chosen file; `None` when the user cancelled or the dialog failed, which is shown.
    pub(super) fn ask_save_file(&mut self, request: &SaveRequest) -> Option<PathBuf> {
        let answer = self.environment.dialogs.save_file(request);
        self.dialog_answer(&request.title, answer).flatten()
    }

    /// The chosen folder; `None` when the user cancelled or the dialog failed, which is shown.
    pub(super) fn ask_folder(&mut self, title: &str) -> Option<PathBuf> {
        let answer = self.environment.dialogs.pick_folder(title);
        self.dialog_answer(title, answer).flatten()
    }

    fn dialog_answer<T>(
        &mut self,
        dialog_title: &str,
        answer: Result<T, DialogError>,
    ) -> Option<T> {
        match answer {
            Ok(answer) => Some(answer),
            Err(error) => {
                let reason = error_chain(error.source.as_ref()).join(": ");
                let context = format!("The \"{dialog_title}\" dialog could not be shown");
                let text = format!("{context}: {reason}");
                self.show_error_logged_as(&context, DIALOG_FAILED_TITLE, text, &error);
                None
            }
        }
    }

    /// A problem found once the window is up, reported like the ones found before it.
    pub fn report_startup_problem(&mut self, problem: StartupProblem) {
        self.report_startup_problems(&[problem]);
    }

    /// Every problem goes to `error.log`; the status line shows the first one.
    fn report_startup_problems(&mut self, problems: &[StartupProblem]) {
        let log_failures: Vec<String> = problems
            .iter()
            .filter_map(|problem| self.record_error(&problem.context, problem.error.as_ref()))
            .collect();
        let Some(first) = problems.first() else {
            return;
        };
        let mut text = format!("{}: {}", first.context, first.error);
        let more = problems.len() - 1;
        if more > 0 {
            text.push_str(&format!(" ({more} more in error.log)"));
        }
        self.set_error_status(text, log_failures.into_iter().next());
    }

    /// Writes the error to `error.log`. When that fails, the failure is returned for the status line.
    pub(super) fn record_error(
        &self,
        context: &str,
        error: &(dyn Error + 'static),
    ) -> Option<String> {
        self.environment
            .error_log
            .record(context, error)
            .err()
            .map(|log_error| format!("{LOG_WRITE_FAILED}: {log_error}"))
    }

    /// Shows `text`, and the failure to log it if there was one, in the status line and in a toast.
    pub(super) fn set_error_status(&mut self, text: String, log_failure: Option<String>) {
        self.report_failure(with_log_failure(text, log_failure));
    }
}

/// `text`, followed by the failure to write it to `error.log` when there was one.
pub(super) fn with_log_failure(text: String, log_failure: Option<String>) -> String {
    match log_failure {
        Some(log_failure) => format!("{text}{STATUS_SEPARATOR}{log_failure}"),
        None => text,
    }
}
