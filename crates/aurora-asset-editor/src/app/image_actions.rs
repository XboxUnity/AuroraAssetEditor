use std::path::PathBuf;

use aurora_asset::{AssetSlot, ScreenshotNumber};

use crate::dialogs::{ChoiceId, Modal, PendingChoice};
use crate::editor::PreparedImage;
use crate::files::{IMAGE_OPEN_DEFAULT_FILTER, file_display_name, image_open_filters};
use crate::jobs::{
    ImageSource, ImportFailure, ImportJob, ImportRequest, ImportResult, Job, JobPriority,
};
use crate::logging::error_chain;
use crate::platform::OpenRequest;
use crate::status::StatusLine;

use super::{App, Page};

pub const ICON_OR_BANNER_TITLE: &str = "Icon or banner?";
pub const REPLACE_SCREENSHOT_TITLE: &str = "Replace screenshot?";
pub const NO_SPACE_TITLE: &str = "No space left";
pub const NO_SPACE_TEXT: &str = "There is no space left for new screenshots :(";
pub const IMAGE_ERROR_TITLE: &str = "Could not use image";
pub const IMAGES_ON_SOURCES_TEXT: &str =
    "Images can only be dropped on Cover, Background, Icon & Banner or Screenshots.";
const ADD_SCREENSHOTS_TITLE: &str = "Select new screenshot(s)";
const SCREENSHOT_DEFAULT_NAME: &str = "screenshot.png";

impl App {
    pub fn selected_screenshot(&self) -> ScreenshotNumber {
        self.selected_screenshot
    }

    pub fn select_screenshot(&mut self, number: ScreenshotNumber) {
        self.selected_screenshot = number;
    }

    /// The page's Replace button; on Screenshots the view passes the selected position, empty or not.
    pub fn replace_image(&mut self, slot: AssetSlot) {
        if self.refuse_while_busy() {
            return;
        }
        let Some((title, default_name)) = replace_dialog(slot) else {
            return;
        };
        let request = image_open_request(title, default_name);
        let Some(path) = self.ask_open_file(&request) else {
            return;
        };
        if let AssetSlot::Screenshot(number) = slot {
            self.selected_screenshot = number;
        }
        self.submit_import(vec![(path, slot)]);
    }

    /// The Screenshots page's Add button: the picked files fill the free positions in order.
    pub fn add_screenshots(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        let request = image_open_request(ADD_SCREENSHOTS_TITLE, SCREENSHOT_DEFAULT_NAME);
        let files = self.ask_open_files(&request);
        self.import_screenshots(files, false);
    }

    /// Empties the slot; there is no confirmation, as with the old Remove of a screenshot.
    pub fn remove_image(&mut self, slot: AssetSlot) {
        if self.refuse_while_busy() {
            return;
        }
        let Some(document) = self.pages.document_for_mut(slot) else {
            return;
        };
        if document.texture_header(slot).is_some() {
            document.remove(slot);
        }
    }

    /// Drag-reorder: the screenshot at `from` lands at `to` and stays selected.
    pub fn move_screenshot(&mut self, from: ScreenshotNumber, to: ScreenshotNumber) {
        if self.refuse_while_busy() {
            return;
        }
        let screenshots = &mut self.pages.screenshots;
        if screenshots
            .texture_header(AssetSlot::Screenshot(from))
            .is_none()
        {
            return;
        }
        screenshots.move_screenshot(from, to);
        self.selected_screenshot = to;
    }

    /// Images dropped on the page on screen, after the job guard in `files_dropped`.
    pub(super) fn drop_images(&mut self, images: Vec<PathBuf>) {
        if images.is_empty() {
            return;
        }
        match self.page {
            Page::Cover => self.import_last_into(images, AssetSlot::Cover),
            Page::Background => self.import_last_into(images, AssetSlot::Background),
            Page::IconBanner => self.ask_icon_or_banner(images),
            Page::Screenshots => self.drop_screenshots(images),
            Page::Online | Page::Console => {
                self.status = StatusLine::info(IMAGES_ON_SOURCES_TEXT);
            }
        }
    }

    /// Carries on after the user answered a choice dialog; `None` is Cancel.
    pub(super) fn answer_choice(&mut self, pending: PendingChoice, choice: Option<ChoiceId>) {
        match pending {
            PendingChoice::IconOrBanner { path, remaining } => {
                match choice {
                    Some(ChoiceId::Icon) => self.submit_import(vec![(path, AssetSlot::Icon)]),
                    Some(ChoiceId::Banner) => self.submit_import(vec![(path, AssetSlot::Banner)]),
                    // Cancel skips this file; the other ids are not offered and `answer_modal` rejects them.
                    None | Some(ChoiceId::Replace | ChoiceId::AddToFreeSlots) => {}
                }
                self.ask_icon_or_banner(remaining);
            }
            PendingChoice::ReplaceScreenshot { first, rest } => {
                let mut files = vec![first];
                files.extend(rest);
                match choice {
                    Some(ChoiceId::Replace) => self.import_screenshots(files, true),
                    Some(ChoiceId::AddToFreeSlots) => self.import_screenshots(files, false),
                    // Cancel loads none of the files; the other ids are not offered and `answer_modal` rejects them.
                    None | Some(ChoiceId::Icon | ChoiceId::Banner) => {}
                }
            }
        }
    }

    /// The old app loaded the files one after another into the same slot, so the last one is what stays.
    fn import_last_into(&mut self, files: Vec<PathBuf>, slot: AssetSlot) {
        if let Some(last) = files.into_iter().next_back() {
            self.submit_import(vec![(last, slot)]);
        }
    }

    fn ask_icon_or_banner(&mut self, files: Vec<PathBuf>) {
        let mut files = files.into_iter();
        let Some(path) = files.next() else {
            return;
        };
        let text = format!(
            "Use {} as the icon or as the banner?",
            file_display_name(&path)
        );
        self.modals.push(Modal::Choice {
            title: ICON_OR_BANNER_TITLE.to_owned(),
            text,
            choices: vec![
                (ChoiceId::Icon, "Icon".to_owned()),
                (ChoiceId::Banner, "Banner".to_owned()),
            ],
            pending: PendingChoice::IconOrBanner {
                path,
                remaining: files.collect(),
            },
        });
    }

    /// Asks once, for the first file, when the selected position is filled.
    fn drop_screenshots(&mut self, images: Vec<PathBuf>) {
        let selected = AssetSlot::Screenshot(self.selected_screenshot);
        let selected_is_filled = self.pages.screenshots.texture_header(selected).is_some();
        if !selected_is_filled {
            self.import_screenshots(images, true);
            return;
        }
        let mut files = images.into_iter();
        let Some(first) = files.next() else {
            return;
        };
        let text = format!(
            "Do you want to replace the current Screenshot with {}?",
            file_display_name(&first)
        );
        self.modals.push(Modal::Choice {
            title: REPLACE_SCREENSHOT_TITLE.to_owned(),
            text,
            choices: vec![
                (ChoiceId::Replace, "Replace".to_owned()),
                (ChoiceId::AddToFreeSlots, "Add to free slots".to_owned()),
            ],
            pending: PendingChoice::ReplaceScreenshot {
                first,
                rest: files.collect(),
            },
        });
    }

    /// Puts the files on screenshot positions in order: the selected position first when `selected_first`
    /// (even when it is empty), then the free ones. Files left over are not imported, and the user is told
    /// once.
    fn import_screenshots(&mut self, files: Vec<PathBuf>, selected_first: bool) {
        let selected = self.selected_screenshot;
        let mut positions = Vec::new();
        if selected_first {
            positions.push(selected);
        }
        positions.extend(
            self.pages
                .screenshots
                .free_screenshots()
                .into_iter()
                .filter(|number| !(selected_first && *number == selected)),
        );
        let no_space_for_all = files.len() > positions.len();
        let placed = files
            .into_iter()
            .zip(positions)
            .map(|(file, number)| (file, AssetSlot::Screenshot(number)))
            .collect();
        self.submit_import(placed);
        if no_space_for_all {
            self.modals.push(Modal::Message {
                title: NO_SPACE_TITLE.to_owned(),
                text: NO_SPACE_TEXT.to_owned(),
            });
        }
    }

    /// Queues one job for the files; the editor stays locked until it ends.
    fn submit_import(&mut self, files: Vec<(PathBuf, AssetSlot)>) {
        if files.is_empty() {
            return;
        }
        let requests = files
            .into_iter()
            .map(|(path, slot)| ImportRequest {
                source: ImageSource::File(path),
                slot,
                resize: true,
            })
            .collect();
        let job = Job::Import(ImportJob {
            requests,
            auto_resize: self.settings.auto_resize,
            compression: self.compression(),
        });
        self.active_work.importing += 1;
        self.environment.jobs.submit(job, JobPriority::User);
    }

    pub(super) fn apply_imported(&mut self, results: Vec<ImportResult>) {
        self.active_work.importing = self.active_work.importing.saturating_sub(1);
        let mut failures = Vec::new();
        for ImportResult {
            slot,
            label,
            result,
        } in results
        {
            match result {
                Ok(image) => self.put_image(slot, image),
                Err(failure) => failures.push((label, failure)),
            }
        }
        self.report_image_failures(failures);
    }

    fn put_image(&mut self, slot: AssetSlot, image: PreparedImage) {
        let Some(document) = self.pages.document_for_mut(slot) else {
            return;
        };
        document.set_prepared(slot, image);
        if let AssetSlot::Screenshot(number) = slot {
            self.selected_screenshot = number;
        }
    }

    /// Every failure is logged; one dialog lists them all. The images already on the page stay.
    fn report_image_failures(&mut self, failures: Vec<(String, ImportFailure)>) {
        let mut lines = Vec::new();
        for (label, failure) in failures {
            let context = format!("Could not use image {label}");
            if let Some(log_failure) = self.record_error(&context, &failure) {
                self.status = StatusLine::error(log_failure);
            }
            let reason = error_chain(&failure).join(": ");
            lines.push(format!(
                "There was an error while trying to process {label}: {reason}"
            ));
        }
        if lines.is_empty() {
            return;
        }
        self.modals.push(Modal::Message {
            title: IMAGE_ERROR_TITLE.to_owned(),
            text: lines.join("\n"),
        });
    }
}

/// The Replace dialog's title and default name; `None` for the unused fourth entry.
fn replace_dialog(slot: AssetSlot) -> Option<(&'static str, &'static str)> {
    match slot {
        AssetSlot::Cover => Some(("Select new cover", "cover.png")),
        AssetSlot::Background => Some(("Select new background", "background.png")),
        AssetSlot::Icon => Some(("Select new icon", "icon.png")),
        AssetSlot::Banner => Some(("Select new banner", "banner.png")),
        AssetSlot::Screenshot(_) => Some(("Select new screenshot", SCREENSHOT_DEFAULT_NAME)),
        AssetSlot::Slot => None,
    }
}

fn image_open_request(title: &str, default_name: &str) -> OpenRequest {
    OpenRequest {
        title: title.to_owned(),
        filters: image_open_filters(),
        default_filter: IMAGE_OPEN_DEFAULT_FILTER,
        default_name: Some(default_name.to_owned()),
    }
}
