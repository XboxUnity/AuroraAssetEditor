use aurora_asset::AssetKind;

use crate::console::page_list;
use crate::dialogs::{ConfirmedAction, Modal};

use super::{App, Replacement};

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
        self.pending_replacements.push_back(replacement);
        self.ask_to_discard(&unsaved, ConfirmedAction::DiscardChanges);
    }

    /// Discard: the replacement the question was asked for goes ahead. Questions are answered in the order they
    /// were asked, so it is the oldest one waiting.
    pub(super) fn discard_changes(&mut self) {
        match self.pending_replacements.pop_front() {
            Some(replacement) => self.run_replacement(replacement),
            None => log::error!("Discard was answered while no replacement was waiting"),
        }
    }

    /// Cancel: every page stays as it is. Files that could not be read are reported as an open reports them;
    /// what was read is dropped unused, so its unreadable images do not matter.
    pub(super) fn keep_changes(&mut self) {
        match self.pending_replacements.pop_front() {
            Some(Replacement::Load(loaded)) => self.report_load_problems(loaded.problems),
            Some(Replacement::Download { .. } | Replacement::NewAssetSet) => {}
            None => log::error!("Cancel was answered while no replacement was waiting"),
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
            text: format!("Discard unsaved changes to {}?", page_list(unsaved)),
            confirm_label: DISCARD_LABEL.to_owned(),
            danger: true,
            cancel_is_default: true,
            action,
        });
    }
}
