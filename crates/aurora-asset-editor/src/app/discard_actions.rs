use aurora_asset::AssetKind;

use crate::console::page_list;
use crate::dialogs::{ConfirmedAction, Modal};

use super::{App, PendingReplacement, Replacement};

pub const DISCARD_TITLE: &str = "Unsaved changes";
pub const DISCARD_LABEL: &str = "Discard";

impl App {
    /// Runs `replacement`, which replaces the pages of `kinds`, once nothing would be lost: at once when none
    /// of them has unsaved changes, otherwise when the user discards those changes.
    pub(super) fn replace_pages(&mut self, kinds: &[AssetKind], replacement: Replacement) {
        let unsaved = self.pages.unsaved_among(kinds);
        if unsaved.is_empty() {
            self.run_replacement(replacement);
            return;
        }
        self.pending_replacements.push_back(PendingReplacement {
            kinds: kinds.to_vec(),
            work: replacement,
        });
        self.ask_to_discard(&unsaved, ConfirmedAction::DiscardChanges);
    }

    /// Discard: the replacement the question was asked for goes ahead. Questions are answered in the order they
    /// were asked, so it is the oldest one waiting.
    pub(super) fn discard_changes(&mut self) {
        match self.pending_replacements.pop_front() {
            Some(pending) => self.run_replacement(pending.work),
            None => log::error!("Discard was answered while no replacement was waiting"),
        }
    }

    /// Cancel: every page stays as it is. Files that could not be read are reported as an open reports them;
    /// what was read is dropped unused, so its unreadable images do not matter.
    pub(super) fn keep_changes(&mut self) {
        let Some(pending) = self.pending_replacements.pop_front() else {
            log::error!("Cancel was answered while no replacement was waiting");
            return;
        };
        match pending.work {
            Replacement::Load(loaded) => self.report_load_problems(loaded.problems),
            Replacement::Download { .. } | Replacement::NewAssetSet => {}
        }
    }

    /// Whether the window may close now. With unsaved changes it asks first, once, and the window stays open
    /// until the user discards them.
    pub fn request_close(&mut self) -> bool {
        if self.close_approved {
            return true;
        }
        let unsaved = self.pages.unsaved_among(&AssetKind::ALL);
        if unsaved.is_empty() {
            return true;
        }
        let asked = self.modals.any(|modal| {
            matches!(
                modal,
                Modal::Confirm {
                    action: ConfirmedAction::CloseWindow,
                    ..
                }
            )
        });
        if !asked {
            self.ask_to_discard(&unsaved, ConfirmedAction::CloseWindow);
        }
        false
    }

    /// The user discarded the unsaved changes to close the window; the window closes next.
    pub fn close_approved(&self) -> bool {
        self.close_approved
    }

    pub(super) fn approve_close(&mut self) {
        self.close_approved = true;
    }

    /// The pages a Discard question names can change after it was asked: the dialog it waited behind may have
    /// saved or replaced them, and a job may land while it is on screen. It names the pages unsaved now; when
    /// none is left, nothing would be lost, so it is not asked and its action runs as Discard would run it.
    pub(super) fn recheck_discard_question(&mut self) {
        while let Some(action) = self.discard_question_on_screen() {
            let kinds = match &action {
                ConfirmedAction::CloseWindow => AssetKind::ALL.to_vec(),
                ConfirmedAction::DiscardChanges => match self.pending_replacements.front() {
                    Some(pending) => pending.kinds.clone(),
                    None => {
                        log::error!("A Discard question is on screen while no replacement waits");
                        return;
                    }
                },
                ConfirmedAction::RemoveFromConsole { .. }
                | ConfirmedAction::UploadWithEmpty { .. } => {
                    return;
                }
            };
            let unsaved = self.pages.unsaved_among(&kinds);
            if !unsaved.is_empty() {
                if let Some(Modal::Confirm { text, .. }) = self.modals.current_mut() {
                    *text = discard_question(&unsaved);
                }
                return;
            }
            self.modals.close_current();
            self.run_confirmed(action);
        }
    }

    /// The action of the Discard question on screen, if one is.
    fn discard_question_on_screen(&self) -> Option<ConfirmedAction> {
        match self.modals.current() {
            Some(Modal::Confirm {
                action: action @ (ConfirmedAction::DiscardChanges | ConfirmedAction::CloseWindow),
                ..
            }) => Some(action.clone()),
            _ => None,
        }
    }

    fn run_replacement(&mut self, replacement: Replacement) {
        match replacement {
            Replacement::Load(loaded) => self.put_loaded(loaded),
            Replacement::Download {
                ids,
                scope,
                after,
                results,
            } => self.put_downloaded(&ids, scope, after, results),
            Replacement::NewAssetSet => self.empty_pages(),
        }
    }

    fn ask_to_discard(&mut self, unsaved: &[AssetKind], action: ConfirmedAction) {
        self.modals.push(Modal::Confirm {
            title: DISCARD_TITLE.to_owned(),
            text: discard_question(unsaved),
            confirm_label: DISCARD_LABEL.to_owned(),
            danger: true,
            cancel_is_default: true,
            action,
        });
    }
}

fn discard_question(unsaved: &[AssetKind]) -> String {
    format!("Discard unsaved changes to {}?", page_list(unsaved))
}
