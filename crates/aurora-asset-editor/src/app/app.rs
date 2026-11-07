use std::error::Error;
use std::path::{Path, PathBuf};

use aurora_asset::ScreenshotNumber;

use crate::dialogs::{Modal, ModalAnswer, ModalField, ModalQueue, UnofferedChoice};
use crate::editor::EditorPages;
use crate::jobs::JobOutcome;
use crate::logging::error_chain;
use crate::platform::{DialogError, OpenRequest, SaveRequest};
use crate::status::{ActiveWork, Locks, StatusLine};

use super::{AppEnvironment, CurrentGame, Page, StartupProblem};

pub const LOG_WRITE_FAILED: &str = "Could not write error.log";
pub const DIALOG_FAILED_TITLE: &str = "Could not open the file dialog";
const STATUS_SEPARATOR: &str = " · ";

/// The app's state and every action the window offers. It holds no UI: the view reads it and calls its
/// methods, and work that may take a while goes to the job runner.
pub struct App {
    pub(super) environment: AppEnvironment,
    pub(super) page: Page,
    pub(super) pages: EditorPages,
    pub(super) current_game: Option<CurrentGame>,
    pub(super) status: StatusLine,
    pub(super) active_work: ActiveWork,
    pub(super) modals: ModalQueue,
    pub(super) selected_screenshot: ScreenshotNumber,
    pub(super) drop_hover: bool,
}

impl App {
    pub fn new(mut environment: AppEnvironment) -> App {
        let startup_problems = std::mem::take(&mut environment.startup_problems);
        let mut app = App {
            environment,
            page: Page::Cover,
            pages: EditorPages::new(),
            current_game: None,
            status: StatusLine::ready(),
            active_work: ActiveWork::default(),
            modals: ModalQueue::new(),
            selected_screenshot: ScreenshotNumber::ALL[0],
            drop_hover: false,
        };
        app.report_startup_problems(&startup_problems);
        app
    }

    pub fn config_dir(&self) -> &Path {
        &self.environment.config_dir
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
        if let Some(rejected) = self.unoffered_choice(answer) {
            self.report_error(
                "A dialog was answered with a button it does not have",
                &rejected,
            );
            return;
        }
        if self.title_id_not_ready(answer) {
            return;
        }
        let Some(modal) = self.modals.close_current() else {
            return;
        };
        match (modal, answer) {
            (Modal::Message { .. }, _) => {}
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

    /// OK on the TitleID prompt does nothing until the form holds eight hex digits; the prompt stays open.
    fn title_id_not_ready(&self, answer: ModalAnswer) -> bool {
        let (ModalAnswer::Accept, Some(Modal::TitleId { form, .. })) =
            (answer, self.modals.current())
        else {
            return false;
        };
        !form.is_valid()
    }

    /// The answer's choice when the dialog on screen does not offer it.
    fn unoffered_choice(&self, answer: ModalAnswer) -> Option<UnofferedChoice> {
        let ModalAnswer::Choice(id) = answer else {
            return None;
        };
        let offered = match self.modals.current()? {
            Modal::Choice { choices, .. } => choices.iter().any(|(offered, _)| *offered == id),
            Modal::Message { .. } | Modal::TitleId { .. } => false,
        };
        if offered {
            None
        } else {
            Some(UnofferedChoice(id))
        }
    }

    pub fn edit_modal_field(&mut self, field: ModalField, text: &str) {
        let Some(Modal::TitleId { form, .. }) = self.modals.current_mut() else {
            return;
        };
        match field {
            ModalField::TitleId => form.edit(text),
        }
    }

    pub fn on_job_outcome(&mut self, outcome: JobOutcome) {
        match outcome {
            JobOutcome::Loaded(loaded) => self.apply_loaded(loaded),
            JobOutcome::Imported(results) => self.apply_imported(results),
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
        if let Some(log_failure) = self.record_error(log_context, error) {
            self.status = StatusLine::error(log_failure);
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

    fn set_error_status(&mut self, text: String, log_failure: Option<String>) {
        let text = match log_failure {
            Some(log_failure) => format!("{text}{STATUS_SEPARATOR}{log_failure}"),
            None => text,
        };
        self.status = StatusLine::error(text);
    }
}
