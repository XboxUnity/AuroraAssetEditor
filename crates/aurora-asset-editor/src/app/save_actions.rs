use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetKind, AssetSlot};
use aurora_console::asset_file_name;
use aurora_image::ExportFormat;

use crate::dialogs::{Modal, TitleIdForm};
use crate::editor::SlotPreview;
use crate::files::{
    asset_save_filter, file_display_name, image_export_filters, save_all_to_folder, save_page,
};
use crate::logging::error_chain;
use crate::platform::SaveRequest;

use super::App;

pub const SAVE_ERROR_TITLE: &str = "Could not save";
pub const SAVE_ALL_TITLE: &str = "Save all assets";
pub const SAVE_ALL_LABEL: &str = "Please specify TitleID:";
pub const SAVE_ALL_FOLDER_TITLE: &str = "Select where to save the asset files";
const ALREADY_SAVED_LABEL: &str = "Already saved:";
const DEFAULT_FILTER: usize = 0;

impl App {
    /// The Save menu's "Save cover…" and its three siblings: one page into one file.
    pub fn save_kind(&mut self, kind: AssetKind) {
        if self.refuse_while_busy() {
            return;
        }
        let request = SaveRequest {
            title: asset_dialog_title(kind).to_owned(),
            filters: vec![asset_save_filter()],
            default_filter: DEFAULT_FILTER,
            default_name: self.default_asset_name(kind),
        };
        let Some(path) = self.ask_save_file(&request) else {
            return;
        };
        if let Err(error) = save_page(self.pages.get_mut(kind), &path) {
            self.show_save_failure(&error.path, &error, &error.source, &[]);
        }
    }

    /// The Save menu's "Save all locally…": asks for the TitleID, then the folder.
    pub fn save_all_locally(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        let prefill = self
            .current_game
            .as_ref()
            .map(|game| game.ids.title_id.to_string())
            .unwrap_or_default();
        self.modals.push(Modal::TitleId {
            title: SAVE_ALL_TITLE.to_owned(),
            label: SAVE_ALL_LABEL.to_owned(),
            form: TitleIdForm::new(&prefill),
        });
    }

    /// OK of the TitleID prompt.
    pub(super) fn save_all_for_form(&mut self, form: &TitleIdForm) {
        let title_id = match form.parse() {
            Ok(title_id) => title_id,
            Err(error) => {
                self.report_error("The TitleID of Save all assets is not valid", &error);
                return;
            }
        };
        let Some(folder) = self.ask_folder(SAVE_ALL_FOLDER_TITLE) else {
            return;
        };
        if let Err(error) = save_all_to_folder(&mut self.pages, &folder, &title_id) {
            self.show_save_failure(&error.path, &error, &error.source, &error.written);
        }
    }

    /// The page's Export button: the slot's pixels into one image file, in the format its extension names.
    pub fn export_image(&mut self, slot: AssetSlot) {
        let Some(document) = self.pages.document_for_mut(slot) else {
            return;
        };
        let SlotPreview::Pixels(pixels) = document.preview(slot) else {
            return;
        };
        let pixels = pixels.clone();
        let Some((title, default_name)) = export_dialog(slot) else {
            return;
        };
        let request = SaveRequest {
            title: title.to_owned(),
            filters: image_export_filters(),
            default_filter: DEFAULT_FILTER,
            default_name: Some(default_name),
        };
        let Some(path) = self.ask_save_file(&request) else {
            return;
        };
        match ExportFormat::from_path(&path).encode(&pixels) {
            Ok(bytes) => {
                if let Err(error) = fs::write(&path, bytes) {
                    self.show_save_failure(&path, &error, &error, &[]);
                }
            }
            Err(error) => self.show_save_failure(&path, &error, &error, &[]),
        }
    }

    fn default_asset_name(&self, kind: AssetKind) -> Option<String> {
        let game = self.current_game.as_ref()?;
        Some(asset_file_name(kind, &game.ids.title_id))
    }

    /// Logs `error` and shows the "Could not save" dialog with `cause` as the reason, naming what Save All had
    /// written before it.
    fn show_save_failure(
        &mut self,
        path: &Path,
        error: &(dyn Error + 'static),
        cause: &(dyn Error + 'static),
        written: &[PathBuf],
    ) {
        let reason = error_chain(cause).join(": ");
        let mut text = format!(
            "There was an error while saving {}: {reason}",
            file_display_name(path)
        );
        if !written.is_empty() {
            let names: Vec<String> = written.iter().map(|path| file_display_name(path)).collect();
            text.push_str(&format!("\n{ALREADY_SAVED_LABEL} {}", names.join(", ")));
        }
        self.show_error(SAVE_ERROR_TITLE, text, error);
    }
}

fn asset_dialog_title(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Cover => "Save cover asset",
        AssetKind::Background => "Save background asset",
        AssetKind::IconBanner => "Save icon & banner asset",
        AssetKind::Screenshots => "Save screenshots asset",
    }
}

/// The export dialog's title and default name; `None` for the unused fourth entry.
fn export_dialog(slot: AssetSlot) -> Option<(&'static str, String)> {
    match slot {
        AssetSlot::Cover => Some(("Select where to save the Cover", "cover.png".to_owned())),
        AssetSlot::Background => Some((
            "Select where to save the Background",
            "background.png".to_owned(),
        )),
        AssetSlot::Icon => Some(("Select where to save the Icon", "icon.png".to_owned())),
        AssetSlot::Banner => Some(("Select where to save the Banner", "banner.png".to_owned())),
        AssetSlot::Screenshot(number) => Some((
            "Select where to save the Screenshot",
            format!("screenshot{}.png", number.get()),
        )),
        AssetSlot::Slot => None,
    }
}
