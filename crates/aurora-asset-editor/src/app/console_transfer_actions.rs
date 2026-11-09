use std::path::Path;

use aurora_asset::AssetKind;
use aurora_console::{ConsoleError, FtpSettings, GameIds, asset_file_name};

use crate::console::{capitalize_first, is_connection_failure, kind_list, kind_name};
use crate::dialogs::{ConfirmedAction, Modal, UploadTargetForm};
use crate::editor::DecodedKind;
use crate::files::slot_label;
use crate::jobs::{
    AfterDownload, AssetUpload, ConsoleJob, DownloadScope, JobPriority, KindDownload,
    TransferReport, UploadPurpose,
};
use crate::status::{ConsoleActivity, STATUS_SEPARATOR, StatusLine};

use super::console_actions::NO_CONSOLE_TEXT;
use super::file_actions::{ASSET_TYPE_COUNT, BUSY_TEXT};
use super::{App, KindChoice, Page, Replacement};

pub const NO_CONSOLE_SET_UP_REASON: &str = "No console set up";
pub const SELECT_A_GAME_REASON: &str = "Select a game first";
pub const UPLOAD_TARGET_TITLE: &str = "Save all assets to console";
pub const REMOVE_CONFIRM_TITLE: &str = "Remove from console?";
pub const EMPTY_UPLOAD_TITLE: &str = "Upload empty assets?";
pub const NOTHING_TO_UPLOAD_TEXT: &str = "Nothing to upload: every page in the editor is empty.";
const REMOVE_CONFIRM_LABEL: &str = "Remove";
const EMPTY_UPLOAD_CONFIRM_LABEL: &str = "Upload";
const ALREADY_SENT_LABEL: &str = "Already sent:";

impl App {
    /// Why the Save menu's "Save all to console…" is disabled, if it is.
    pub fn save_to_console_disabled_reason(&self) -> Option<&'static str> {
        match self.console.settings {
            Some(_) => None,
            None => Some(NO_CONSOLE_SET_UP_REASON),
        }
    }

    /// Why the console page's per-game actions are disabled, if they are. They act on the current game, so
    /// its row must be selected and shown: a filter that hides it, or a refresh, disables them.
    pub fn console_actions_disabled_reason(&self) -> Option<&'static str> {
        let row_shown = self.console.table.selected_visible_row().is_some();
        match self.current_game {
            Some(_) if row_shown => None,
            Some(_) | None => Some(SELECT_A_GAME_REASON),
        }
    }

    /// "Open in editor": the selected game's full set replaces every page, then the first page that received
    /// content opens.
    pub fn open_in_editor(&mut self) {
        self.start_download(DownloadScope::FullSet, AfterDownload::OpenFirstLoaded);
    }

    /// A double-click on a row, or Return on the selected one: selects the row's game and opens it like
    /// "Open in editor", refused the same way while the console or the editor is busy.
    pub fn open_game_in_editor(&mut self, visible_row: usize) {
        self.select_game(visible_row);
        if self.console.table.selected_visible_row() == Some(visible_row) {
            self.open_in_editor();
        }
    }

    /// "Download": `All` replaces every page with the full set and stays on the console page; one kind
    /// replaces and opens its page.
    pub fn download(&mut self, choice: KindChoice) {
        let after = match choice {
            KindChoice::All => AfterDownload::StayOnConsole,
            KindChoice::One(_) => AfterDownload::OpenFirstLoaded,
        };
        self.start_download(DownloadScope::from(choice), after);
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
            self.start_upload(ids, files, UploadPurpose::UploadCurrent);
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
            cancel_is_default: false,
            action: ConfirmedAction::RemoveFromConsole { ids, kinds },
        });
    }

    /// The Save menu's "Save all to console…": uploads the pages that have content to the current game
    /// straight away; without one, it first asks which game, typed or picked from the console's games.
    pub fn save_all_to_console(&mut self) {
        if self.save_to_console_disabled_reason().is_some() {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        }
        if self.refuse_console_transfer() {
            return;
        }
        match &self.current_game {
            Some(game) => {
                let ids = game.ids.clone();
                self.save_all_to(ids);
            }
            None => {
                self.modals.push(Modal::UploadTarget {
                    title: UPLOAD_TARGET_TITLE.to_owned(),
                    form: UploadTargetForm::new(),
                });
                self.load_games_for_picker();
            }
        }
    }

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
        self.save_all_to(ids);
    }

    /// Uploads the pages that have content to the game with `ids`; pages without content are skipped.
    fn save_all_to(&mut self, ids: GameIds) {
        let kinds_with_content: Vec<AssetKind> = AssetKind::ALL
            .into_iter()
            .filter(|kind| self.pages.get(*kind).has_content())
            .collect();
        if kinds_with_content.is_empty() {
            self.status = StatusLine::info(NOTHING_TO_UPLOAD_TEXT);
            return;
        }
        let files = self.page_files(&kinds_with_content);
        self.start_upload(ids, files, UploadPurpose::SaveAll);
    }

    pub(super) fn run_confirmed(&mut self, action: ConfirmedAction) {
        match action {
            ConfirmedAction::RemoveFromConsole { ids, kinds } => self.start_remove(ids, kinds),
            ConfirmedAction::UploadWithEmpty { ids, files } => {
                self.start_upload(ids, files, UploadPurpose::UploadCurrent);
            }
            ConfirmedAction::DiscardChanges => self.discard_changes(),
            ConfirmedAction::CloseWindow => self.approve_close(),
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
    pub(super) fn refuse_console_transfer(&mut self) -> bool {
        if !self.locks().console_editor_writes {
            return false;
        }
        self.status = StatusLine::info(BUSY_TEXT);
        true
    }

    /// The bytes an upload of the page sends: its asset file as it is, gaps and offsets kept.
    pub fn page_bytes(&mut self, kind: AssetKind) -> Vec<u8> {
        self.pages.get_mut(kind).to_bytes()
    }

    fn page_files(&mut self, kinds: &[AssetKind]) -> Vec<AssetUpload> {
        kinds
            .iter()
            .map(|kind| AssetUpload {
                kind: *kind,
                bytes: self.page_bytes(*kind),
                change_count: self.pages.get(*kind).change_count(),
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
            cancel_is_default: false,
            action: ConfirmedAction::UploadWithEmpty { ids, files },
        });
    }

    fn start_download(&mut self, scope: DownloadScope, after: AfterDownload) {
        let Some((settings, ids)) = self.console_target() else {
            return;
        };
        if self.refuse_while_busy() || self.refuse_while_console_busy() {
            return;
        }
        self.active_work.console = Some(ConsoleActivity::Downloading);
        let job = ConsoleJob::Download { ids, scope, after };
        self.submit_console_job(settings, job, JobPriority::User);
    }

    fn start_upload(&mut self, ids: GameIds, uploads: Vec<AssetUpload>, purpose: UploadPurpose) {
        let Some(settings) = self.console.settings.clone() else {
            self.status = StatusLine::info(NO_CONSOLE_TEXT);
            return;
        };
        if self.refuse_while_console_busy() {
            return;
        }
        self.active_work.console = Some(ConsoleActivity::Uploading);
        self.upload_change_counts = uploads
            .iter()
            .map(|upload| (upload.kind, upload.change_count))
            .collect();
        let job = ConsoleJob::Upload {
            ids,
            uploads,
            purpose,
        };
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

    /// When pages the download replaces hold unsaved changes, the user is asked first.
    pub(super) fn apply_downloaded(
        &mut self,
        ids: GameIds,
        scope: DownloadScope,
        after: AfterDownload,
        results: Vec<KindDownload>,
    ) {
        self.active_work.console = None;
        // The download is over: its progress leaves the status line, also while the question waits.
        self.status.progress = None;
        let replaced = replaced_by_download(scope, &results);
        let download = Replacement::Download {
            ids,
            scope,
            after,
            results,
        };
        self.replace_pages(&replaced, download);
    }

    pub(super) fn put_downloaded(
        &mut self,
        ids: &GameIds,
        scope: DownloadScope,
        after: AfterDownload,
        results: Vec<KindDownload>,
    ) {
        match scope {
            DownloadScope::OneKind(_) => self.apply_kind_download(ids, after, results),
            DownloadScope::FullSet => self.apply_full_set(ids, after, results),
        }
    }

    /// One kind: its page takes what arrived; a kind the console does not have leaves the page as it is.
    fn apply_kind_download(
        &mut self,
        ids: &GameIds,
        after: AfterDownload,
        results: Vec<KindDownload>,
    ) {
        for KindDownload { kind, result } in results {
            match result {
                Ok(Some(decoded)) => {
                    self.replace_pages_with_downloads(ids, vec![*decoded]);
                    if after == AfterDownload::OpenFirstLoaded {
                        self.page = Page::of_kind(kind);
                    }
                    self.report_done(format!("Downloaded {} from the console", kind_name(kind)));
                }
                Ok(None) => {
                    self.status =
                        StatusLine::info(format!("No {} asset on console", kind_name(kind)));
                }
                Err(error) => self.report_download_failure(kind, &error),
            }
        }
    }

    /// The full set: the pages change only once every kind has arrived, so the editor never mixes the
    /// console's assets with what was open before. A kind the console does not have becomes an empty page.
    fn apply_full_set(&mut self, ids: &GameIds, after: AfterDownload, results: Vec<KindDownload>) {
        let mut found = Vec::new();
        for KindDownload { kind, result } in results {
            match result {
                Ok(Some(decoded)) => found.push(*decoded),
                Ok(None) => {}
                Err(error) => {
                    self.report_download_failure(kind, &error);
                    return;
                }
            }
        }
        let first_found = found.first().map(|decoded| Page::of_kind(decoded.kind));
        for kind in AssetKind::ALL {
            if !found.iter().any(|decoded| decoded.kind == kind) {
                self.pages.get_mut(kind).clear();
            }
        }
        self.replace_pages_with_downloads(ids, found);
        if after == AfterDownload::OpenFirstLoaded {
            self.page = first_found.unwrap_or(Page::Cover);
        }
        let present = self.pages.present_types();
        self.report_done(format!(
            "Loaded {} from the console{STATUS_SEPARATOR}{present} of {ASSET_TYPE_COUNT} asset types present",
            self.game_label(ids)
        ));
    }

    /// Each downloaded page replaces its page in the editor; entries that could not be decoded are listed.
    fn replace_pages_with_downloads(&mut self, ids: &GameIds, downloads: Vec<DecodedKind>) {
        let mut unread_images = Vec::new();
        for decoded in downloads {
            let file_name = asset_file_name(decoded.kind, &ids.title_id);
            for dropped in &decoded.dropped {
                self.note_unread_image(
                    Path::new(&file_name),
                    &slot_label(dropped.slot),
                    &dropped.error,
                    &mut unread_images,
                );
            }
            self.pages.get_mut(decoded.kind).replace_with(decoded);
        }
        self.show_unread_images(unread_images);
    }

    fn report_download_failure(&mut self, kind: AssetKind, error: &ConsoleError) {
        if is_connection_failure(error) {
            self.show_connection_failed(error);
        } else {
            let context = format!("Could not download {}", kind_name(kind));
            self.report_console_error(&context, error);
        }
    }

    /// How a status line names the game with `ids`: "{title} ({TitleID})", or the TitleID alone when neither
    /// the current game nor the console's list knows its title.
    pub(super) fn game_label(&self, ids: &GameIds) -> String {
        let current_title = self
            .current_game
            .as_ref()
            .filter(|game| &game.ids == ids)
            .map(|game| game.title.as_str());
        let listed_title = || {
            self.console
                .table
                .game(ids)
                .map(|game| game.title_name.as_str())
        };
        match current_title.or_else(listed_title) {
            Some(title) => format!("{title} ({})", ids.title_id),
            None => ids.title_id.to_string(),
        }
    }

    pub(super) fn apply_uploaded(&mut self, report: TransferReport, purpose: UploadPurpose) {
        self.mark_uploaded_pages_saved(&report.done);
        let success = match purpose {
            UploadPurpose::UploadCurrent => {
                format!("Uploaded {} to the console", kind_list(&report.done))
            }
            UploadPurpose::SaveAll => {
                saved_to_game_text(report.done.len(), &self.game_label(&report.ids))
            }
        };
        self.apply_transferred(report, "upload", success);
    }

    /// A page whose bytes reached the console is saved, unless it changed while they were on their way.
    fn mark_uploaded_pages_saved(&mut self, done: &[AssetKind]) {
        let sent = std::mem::take(&mut self.upload_change_counts);
        for kind in done {
            match sent.iter().find(|(sent_kind, _)| sent_kind == kind) {
                Some((_, change_count)) => self.pages.get_mut(*kind).mark_saved(*change_count),
                None => log::error!("The console reports {kind:?} uploaded, which was not sent"),
            }
        }
    }

    pub(super) fn apply_removed(&mut self, report: TransferReport) {
        let success = format!("Removed {} from the console", kind_list(&report.done));
        self.apply_transferred(report, "remove", success);
    }

    /// Shows the result, `success` when every kind went, and checks the game's assets on the console again,
    /// since they changed.
    fn apply_transferred(&mut self, report: TransferReport, verb: &str, success: String) {
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
                if done.is_empty() {
                    self.report_console_error(&context, &error);
                } else {
                    let note = format!("{ALREADY_SENT_LABEL} {}", kind_list(&done));
                    self.report_console_error_noting(&context, &error, &note);
                }
            }
            None => self.report_done(success),
        }
    }
}

/// The pages a download replaces: one kind's when it arrived; with the full set every page, unless a kind
/// failed, which changes none.
fn replaced_by_download(scope: DownloadScope, results: &[KindDownload]) -> Vec<AssetKind> {
    match scope {
        DownloadScope::OneKind(_) => results
            .iter()
            .filter(|download| matches!(download.result, Ok(Some(_))))
            .map(|download| download.kind)
            .collect(),
        DownloadScope::FullSet if results.iter().any(|download| download.result.is_err()) => {
            Vec::new()
        }
        DownloadScope::FullSet => AssetKind::ALL.to_vec(),
    }
}

/// "Saved 1 asset type to {game}", "Saved {count} asset types to {game}".
fn saved_to_game_text(count: usize, game: &str) -> String {
    let asset_types = if count == 1 {
        "asset type"
    } else {
        "asset types"
    };
    format!("Saved {count} {asset_types} to {game}")
}

fn pronoun_for(kind_count: usize) -> &'static str {
    if kind_count == 1 { "it" } else { "them" }
}
