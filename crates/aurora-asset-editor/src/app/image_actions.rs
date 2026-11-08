use std::path::PathBuf;

use aurora_asset::{AssetSlot, ScreenshotNumber};

use crate::dialogs::{ChoiceId, Modal, PendingChoice};
use crate::editor::PreparedImage;
use crate::files::{IMAGE_OPEN_DEFAULT_FILTER, file_display_name, image_open_filters};
use crate::jobs::{
    ImageSource, ImportFailure, ImportJob, ImportRequest, ImportResult, ImportSlot, Job,
    JobPriority,
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
const FIRST_FREE_SCREENSHOT: ImportSlot = ImportSlot::FreeScreenshot { preferred: None };
const ADD_SCREENSHOTS_TITLE: &str = "Select new screenshot(s)";
const SCREENSHOT_DEFAULT_NAME: &str = "screenshot.png";

impl App {
    pub fn selected_screenshot(&self) -> ScreenshotNumber {
        self.selected_screenshot
    }

    pub fn select_screenshot(&mut self, number: ScreenshotNumber) {
        if self.refuse_while_busy() {
            return;
        }
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
        self.submit_import(vec![(path, ImportSlot::Slot(slot))]);
    }

    /// The Screenshots page's Add button: the picked files fill the free positions in order.
    pub fn add_screenshots(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        let request = image_open_request(ADD_SCREENSHOTS_TITLE, SCREENSHOT_DEFAULT_NAME);
        let files = self.ask_open_files(&request);
        self.import_screenshots(files, FIRST_FREE_SCREENSHOT);
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
                    Some(ChoiceId::Icon) => {
                        self.submit_import(vec![(path, ImportSlot::Slot(AssetSlot::Icon))]);
                    }
                    Some(ChoiceId::Banner) => {
                        self.submit_import(vec![(path, ImportSlot::Slot(AssetSlot::Banner))]);
                    }
                    // Cancel skips this file; the other ids are not offered and `answer_modal` rejects them.
                    None | Some(ChoiceId::Replace | ChoiceId::AddToFreeSlots) => {}
                }
                self.ask_icon_or_banner(remaining);
            }
            PendingChoice::ReplaceScreenshot { first, rest } => {
                let mut files = vec![first];
                files.extend(rest);
                match choice {
                    Some(ChoiceId::Replace) => {
                        let selected = AssetSlot::Screenshot(self.selected_screenshot);
                        self.import_screenshots(files, ImportSlot::Slot(selected));
                    }
                    Some(ChoiceId::AddToFreeSlots) => {
                        self.import_screenshots(files, FIRST_FREE_SCREENSHOT);
                    }
                    // Cancel loads none of the files; the other ids are not offered and `answer_modal` rejects them.
                    None | Some(ChoiceId::Icon | ChoiceId::Banner) => {}
                }
            }
        }
    }

    /// The old app loaded the files one after another into the same slot, so the last one is what stays.
    fn import_last_into(&mut self, files: Vec<PathBuf>, slot: AssetSlot) {
        if let Some(last) = files.into_iter().next_back() {
            self.submit_import(vec![(last, ImportSlot::Slot(slot))]);
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
            let first = ImportSlot::FreeScreenshot {
                preferred: Some(self.selected_screenshot),
            };
            self.import_screenshots(images, first);
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

    /// Puts the first file on `first`, the others on the positions still free when the images reach the page.
    fn import_screenshots(&mut self, files: Vec<PathBuf>, first: ImportSlot) {
        let slots = std::iter::once(first).chain(std::iter::repeat(FIRST_FREE_SCREENSHOT));
        self.submit_import(files.into_iter().zip(slots).collect());
    }

    /// Queues one job for the files; the editor stays locked until it ends.
    fn submit_import(&mut self, files: Vec<(PathBuf, ImportSlot)>) {
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

    /// Images without a free screenshot position left are not put on the page, and the user is told once.
    pub(super) fn apply_imported(&mut self, results: Vec<ImportResult>) {
        self.active_work.importing = self.active_work.importing.saturating_sub(1);
        let pending_status = self.pending_import_status.take();
        let mut failures = Vec::new();
        let mut out_of_space = false;
        for ImportResult {
            slot,
            label,
            result,
        } in results
        {
            match (result, self.place(slot)) {
                (Ok(image), Some(slot)) => self.put_image(slot, image),
                (Ok(_), None) => out_of_space = true,
                (Err(failure), _) => failures.push((label, failure)),
            }
        }
        if out_of_space {
            self.modals.push(Modal::Message {
                title: NO_SPACE_TITLE.to_owned(),
                text: NO_SPACE_TEXT.to_owned(),
            });
        }
        if let Some(pending) = pending_status {
            self.status = if failures.is_empty() {
                StatusLine::info(pending.done)
            } else {
                StatusLine::error(pending.failed)
            };
        }
        self.report_image_failures(failures);
    }

    /// The slot an imported image goes to now; `None` when no screenshot position is free.
    fn place(&self, slot: ImportSlot) -> Option<AssetSlot> {
        match slot {
            ImportSlot::Slot(slot) => Some(slot),
            ImportSlot::FreeScreenshot { preferred } => {
                let free = self.pages.screenshots.free_screenshots();
                preferred
                    .filter(|number| free.contains(number))
                    .or_else(|| free.first().copied())
                    .map(AssetSlot::Screenshot)
            }
        }
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
                self.report_failure(log_failure);
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
