use std::path::Path;

use aurora_asset::AssetKind;
use aurora_console::{FtpSettings, GameIds, asset_file_name};

use crate::console::{capitalize_first, is_connection_failure, kind_list, kind_name};
use crate::dialogs::{ConfirmedAction, Modal, UploadTargetForm};
use crate::files::slot_label;
use crate::jobs::{
    AfterDownload, AssetUpload, ConsoleJob, JobPriority, KindDownload, TransferReport,
};
use crate::status::{ConsoleActivity, StatusLine};

use super::console_actions::NO_CONSOLE_TEXT;
use super::file_actions::BUSY_TEXT;
use super::{App, KindChoice, Page};

pub const NO_CONSOLE_SET_UP_REASON: &str = "No console set up";
pub const SELECT_A_GAME_REASON: &str = "Select a game first";
pub const UPLOAD_TARGET_TITLE: &str = "Save all assets to console";
pub const REMOVE_CONFIRM_TITLE: &str = "Remove from console?";
pub const EMPTY_UPLOAD_TITLE: &str = "Upload empty assets?";
pub const NOTHING_TO_UPLOAD_TEXT: &str = "Nothing to upload: every page in the editor is empty.";
const REMOVE_CONFIRM_LABEL: &str = "Remove";
const EMPTY_UPLOAD_CONFIRM_LABEL: &str = "Upload";
const STATUS_SEPARATOR: &str = " · ";

impl App {
    /// Why the Save menu's "Save all to console…" is disabled, if it is.
    pub fn save_to_console_disabled_reason(&self) -> Option<&'static str> {
        match self.console.settings {
            Some(_) => None,
            None => Some(NO_CONSOLE_SET_UP_REASON),
        }
    }

    /// Why the console page's per-game actions are disabled, if they are.
    pub fn console_actions_disabled_reason(&self) -> Option<&'static str> {
        match self.current_game {
            Some(_) => None,
            None => Some(SELECT_A_GAME_REASON),
        }
    }

    /// "Open in editor": every kind of the selected game, then the first page that received content.
    pub fn open_in_editor(&mut self) {
        self.start_download(KindChoice::All, AfterDownload::OpenFirstLoaded);
    }

    /// "Download": `All` stays on the console page, one kind opens its page.
    pub fn download(&mut self, choice: KindChoice) {
        let after = match choice {
            KindChoice::All => AfterDownload::StayOnConsole,
            KindChoice::One(_) => AfterDownload::OpenFirstLoaded,
        };
        self.start_download(choice, after);
    }

    /// "Upload current": the editor's pages as they are, to the selected game. Empty pages ask first, since
    /// uploading one removes that asset from the console.
    pub fn upload_current(&mut self, choice: KindChoice) {
        let Some((_, ids)) = self.console_target() else {
            return;
        };
        if self.refuse_console_transfer() {
            return;
        }
        let kinds = choice.kinds();
        let empty: Vec<AssetKind> = kinds
            .iter()
            .copied()
            .filter(|kind| !self.pages.get(*kind).has_content())
            .collect();
        let files = self.page_files(&kinds);
        if empty.is_empty() {
            self.start_upload(ids, files);
        } else {
            self.ask_to_upload_empty(ids, files, &empty);
        }
    }

    /// "Remove": asks, then replaces the console's files with the empty asset. The editor is untouched.
    pub fn remove_from_console(&mut self, choice: KindChoice) {
        let Some((_, ids)) = self.console_target() else {
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        let kinds = choice.kinds();
        let title = self
            .current_game
            .as_ref()
            .map(|game| game.title.clone())
            .unwrap_or_default();
        let text = format!(
            "Remove the {} of {title} ({}) from the console? Aurora will no longer show {}.",
            kind_list(&kinds),
            ids.title_id,
            pronoun_for(kinds.len())
        );
        self.modals.push(Modal::Confirm {
            title: REMOVE_CONFIRM_TITLE.to_owned(),
            text,
            confirm_label: REMOVE_CONFIRM_LABEL.to_owned(),
            danger: true,
            action: ConfirmedAction::RemoveFromConsole { ids, kinds },
        });
    }

    /// The Save menu's "Save all to console…": asks which game, then uploads the pages that have content.
    pub fn save_all_to_console(&mut self) {
        if self.save_to_console_disabled_reason().is_some() {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        }
        if self.refuse_console_transfer() {
            return;
        }
        let form = UploadTargetForm::new(self.current_game.as_ref().map(|game| &game.ids));
        self.modals.push(Modal::UploadTarget {
            title: UPLOAD_TARGET_TITLE.to_owned(),
            form,
        });
    }

    /// OK of the upload target dialog.
    pub(super) fn save_all_to_console_for_form(&mut self, form: &UploadTargetForm) {
        let ids = match form.parse() {
            Ok(ids) => ids,
            Err(error) => {
                self.report_error(
                    "The IDs of Save all assets to console are not valid",
                    &error,
                );
                return;
            }
        };
        if self.refuse_console_transfer() {
            return;
        }
        let kinds_with_content: Vec<AssetKind> = AssetKind::ALL
            .into_iter()
            .filter(|kind| self.pages.get(*kind).has_content())
            .collect();
        if kinds_with_content.is_empty() {
            self.status = StatusLine::info(NOTHING_TO_UPLOAD_TEXT);
            return;
        }
        let files = self.page_files(&kinds_with_content);
        self.start_upload(ids, files);
    }

    pub(super) fn run_confirmed(&mut self, action: ConfirmedAction) {
        match action {
            ConfirmedAction::RemoveFromConsole { ids, kinds } => self.start_remove(ids, kinds),
            ConfirmedAction::UploadWithEmpty { ids, files } => self.start_upload(ids, files),
        }
    }

    /// The settings and the selected game a per-game console action needs; says what is missing otherwise.
    fn console_target(&mut self) -> Option<(FtpSettings, GameIds)> {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return None;
        };
        let Some(game) = &self.current_game else {
            self.status = StatusLine::info(SELECT_A_GAME_REASON);
            return None;
        };
        Some((settings, game.ids.clone()))
    }

    /// An upload reads the pages, so it waits for whatever writes them as well as for the console.
    fn refuse_console_transfer(&mut self) -> bool {
        if !self.locks().console_editor_writes {
            return false;
        }
        self.status = StatusLine::info(BUSY_TEXT);
        true
    }

    fn page_files(&mut self, kinds: &[AssetKind]) -> Vec<AssetUpload> {
        kinds
            .iter()
            .map(|kind| AssetUpload {
                kind: *kind,
                bytes: self.pages.get_mut(*kind).to_bytes(),
            })
            .collect()
    }

    fn ask_to_upload_empty(&mut self, ids: GameIds, files: Vec<AssetUpload>, empty: &[AssetKind]) {
        let verb = if empty.len() == 1 { "is" } else { "are" };
        let pronoun = pronoun_for(empty.len());
        let text = format!(
            "{} {verb} empty in the editor. Uploading replaces {pronoun} on the console with an empty asset, which removes {pronoun}.",
            capitalize_first(&kind_list(empty))
        );
        self.modals.push(Modal::Confirm {
            title: EMPTY_UPLOAD_TITLE.to_owned(),
            text,
            confirm_label: EMPTY_UPLOAD_CONFIRM_LABEL.to_owned(),
            danger: false,
            action: ConfirmedAction::UploadWithEmpty { ids, files },
        });
    }

    fn start_download(&mut self, choice: KindChoice, after: AfterDownload) {
        let Some((settings, ids)) = self.console_target() else {
            return;
        };
        if self.refuse_while_busy() || self.refuse_while_console_busy() {
            return;
        }
        self.active_work.console = Some(ConsoleActivity::Downloading);
        let job = ConsoleJob::Download {
            ids,
            kinds: choice.kinds(),
            after,
        };
        self.submit_console_job(settings, job, JobPriority::User);
    }

    fn start_upload(&mut self, ids: GameIds, uploads: Vec<AssetUpload>) {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        self.active_work.console = Some(ConsoleActivity::Uploading);
        let job = ConsoleJob::Upload { ids, uploads };
        self.submit_console_job(settings, job, JobPriority::User);
    }

    fn start_remove(&mut self, ids: GameIds, kinds: Vec<AssetKind>) {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        self.active_work.console = Some(ConsoleActivity::Removing);
        let job = ConsoleJob::Remove { ids, kinds };
        self.submit_console_job(settings, job, JobPriority::User);
    }

    pub(super) fn apply_downloaded(
        &mut self,
        ids: &GameIds,
        after: AfterDownload,
        results: Vec<KindDownload>,
    ) {
        self.active_work.console = None;
        let mut loaded = Vec::new();
        let mut missing = Vec::new();
        let mut failure = None;
        let mut unread_images = Vec::new();
        for KindDownload { kind, result } in results {
            match result {
                Ok(Some(decoded)) => {
                    let file_name = asset_file_name(kind, &ids.title_id);
                    for dropped in &decoded.dropped {
                        self.note_unread_image(
                            Path::new(&file_name),
                            &slot_label(dropped.slot),
                            &dropped.error,
                            &mut unread_images,
                        );
                    }
                    self.pages.get_mut(kind).replace_with(*decoded);
                    loaded.push(kind);
                }
                Ok(None) => missing.push(kind),
                Err(error) => failure = Some((kind, error)),
            }
        }
        self.show_unread_images(unread_images);
        if let (AfterDownload::OpenFirstLoaded, Some(first)) = (after, loaded.first()) {
            self.page = Page::of_kind(*first);
        }
        if let Some((kind, error)) = failure {
            if is_connection_failure(&error) {
                self.show_connection_failed(&error);
            } else {
                let context = format!("Could not download {}", kind_name(kind));
                self.report_console_error(&context, &error);
            }
            return;
        }
        let mut parts = Vec::new();
        if !loaded.is_empty() {
            parts.push(format!(
                "Downloaded {} from the console",
                kind_list(&loaded)
            ));
        }
        for kind in missing {
            parts.push(format!("No {} asset on console", kind_name(kind)));
        }
        self.status = StatusLine::info(parts.join(STATUS_SEPARATOR));
    }

    pub(super) fn apply_uploaded(&mut self, report: TransferReport) {
        self.apply_transferred(report, "upload", "Uploaded", "to");
    }

    pub(super) fn apply_removed(&mut self, report: TransferReport) {
        self.apply_transferred(report, "remove", "Removed", "from");
    }

    /// Shows the result and checks the game's assets on the console again, since they changed.
    fn apply_transferred(
        &mut self,
        report: TransferReport,
        verb: &str,
        past_tense: &str,
        preposition: &str,
    ) {
        self.active_work.console = None;
        let TransferReport { ids, done, failure } = report;
        if !done.is_empty() {
            self.console.presence_gate().mark_reachable();
            self.queue_presence_check(&ids);
        }
        match failure {
            Some((_, error)) if is_connection_failure(&error) => {
                self.show_connection_failed(&error);
            }
            Some((kind, error)) => {
                let context = format!("Could not {verb} {}", kind_name(kind));
                self.report_console_error(&context, &error);
            }
            None => {
                self.status = StatusLine::info(format!(
                    "{past_tense} {} {preposition} the console",
                    kind_list(&done)
                ));
            }
        }
    }
}

fn pronoun_for(kind_count: usize) -> &'static str {
    if kind_count == 1 { "it" } else { "them" }
}
