use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetKind, AssetSlot};

use crate::dialogs::Modal;
use crate::editor::{DecodedKind, EditorPages, PreparedImage};
use crate::files::{
    ASSET_OPEN_DEFAULT_FILTER, FileKind, FileProblem, FsdImages, LoadProblem, LoadedFile,
    LoadedFiles, asset_open_filters, detect, file_display_name, slot_label,
};
use crate::jobs::{Job, JobPriority, LoadJob};
use crate::logging::error_chain;
use crate::platform::OpenRequest;
use crate::status::{STATUS_SEPARATOR, StatusLine};

use super::{App, Page};

pub const OPEN_DIALOG_TITLE: &str = "Select Asset(s) to load";
pub const LOAD_PROBLEMS_TITLE: &str = "Could not load files";
pub const NO_ASSETS_TITLE: &str = "No assets";
pub const UNREAD_IMAGES_TITLE: &str = "Some images could not be read";
pub const NOT_ENOUGH_SPACE_TITLE: &str = "Not enough space";
pub const NOT_ENOUGH_SPACE_TEXT: &str = "Not enough space to fit all screenshots, please clear current screenshots and load the FSD asset again...";
const LOAD_PROBLEMS_FOOTER: &str = "Details are in error.log.";
pub(super) const ASSET_TYPE_COUNT: usize = 5;
pub const BUSY_TEXT: &str = "Busy, try again when the current work finishes";
pub const DROP_DURING_QUESTION_TEXT: &str =
    "Finish the open question first, then drop the files again.";
const MAGIC_LEN: u64 = 4;

impl App {
    /// The Open menu's "Open asset files…".
    pub fn open_files_with_dialog(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        let request = OpenRequest {
            title: OPEN_DIALOG_TITLE.to_owned(),
            filters: asset_open_filters(),
            default_filter: ASSET_OPEN_DEFAULT_FILTER,
            default_name: None,
        };
        let paths = self.ask_open_files(&request);
        self.open_paths(paths);
    }

    /// Loads files given on the command line or dropped on the window. Their content decides what they are.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() || self.refuse_while_busy() {
            return;
        }
        let compression = self.compression();
        let job = Job::Load(LoadJob { paths, compression });
        self.active_work.loading += 1;
        self.environment.jobs.submit(job, JobPriority::User);
    }

    /// Files dropped on the window. `.asset` and FSD files load on any page, images go where the page on
    /// screen puts them (the OS does not say where on the window a file landed). While a dialog is open they
    /// are refused: its answer may change where they would go.
    pub fn files_dropped(&mut self, paths: Vec<PathBuf>) {
        self.drop_hover = false;
        if paths.is_empty() {
            return;
        }
        if self.modals.current().is_some() {
            self.status = StatusLine::info(DROP_DURING_QUESTION_TEXT);
            return;
        }
        if self.refuse_while_busy() {
            return;
        }
        let (images, loadable): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| is_dropped_image(path));
        self.open_paths(loadable);
        self.drop_images(images);
    }

    pub fn set_drop_hover(&mut self, hover: bool) {
        self.drop_hover = hover;
    }

    /// Whether files are being dragged over the window.
    pub fn drop_hover(&self) -> bool {
        self.drop_hover
    }

    /// The Open menu's "New asset set": empty pages, the current game stays.
    pub fn new_asset_set(&mut self) {
        if self.refuse_while_busy() {
            return;
        }
        self.pages.reset();
        self.status = StatusLine::ready();
    }

    /// The old busy indicator blocked all input while a load ran; here the request is refused and says so.
    pub(super) fn refuse_while_busy(&mut self) -> bool {
        if !self.locks().editor {
            return false;
        }
        self.status = StatusLine::info(BUSY_TEXT);
        true
    }

    pub(super) fn apply_loaded(&mut self, loaded: LoadedFiles) {
        self.active_work.loading = self.active_work.loading.saturating_sub(1);
        let mut unread_images = Vec::new();
        let mut last_loaded = None;
        for file in loaded.files {
            let (path, contributed) = match file {
                LoadedFile::Asset { path, decoded } => {
                    self.apply_asset(&path, *decoded, &mut unread_images);
                    (path, true)
                }
                LoadedFile::Fsd { path, images } => {
                    let received_any = self.apply_fsd(&path, *images, &mut unread_images);
                    (path, received_any)
                }
            };
            if contributed {
                last_loaded = Some(file_display_name(&path));
            }
        }
        if let Some(name) = last_loaded {
            let present = self.pages.present_types();
            self.status = StatusLine::info(format!(
                "Loaded {name}{STATUS_SEPARATOR}{present} of {ASSET_TYPE_COUNT} asset types present"
            ));
        }
        self.show_unread_images(unread_images);
        self.report_load_problems(loaded.problems);
    }

    fn apply_asset(&mut self, path: &Path, decoded: DecodedKind, unread_images: &mut Vec<String>) {
        let kind = decoded.kind;
        for dropped in &decoded.dropped {
            let entry = slot_label(dropped.slot);
            self.note_unread_image(path, &entry, &dropped.error, unread_images);
        }
        self.pages.get_mut(kind).replace_with(decoded);
        self.page = Page::of_kind(kind);
    }

    /// Whether any image reached a page.
    fn apply_fsd(
        &mut self,
        path: &Path,
        images: FsdImages,
        unread_images: &mut Vec<String>,
    ) -> bool {
        for (entry, error) in &images.unreadable {
            self.note_unread_image(path, entry, error, unread_images);
        }
        let mut received = FsdReceived::default();
        received.put(
            &mut self.pages,
            AssetKind::Cover,
            AssetSlot::Cover,
            images.cover,
        );
        received.put(
            &mut self.pages,
            AssetKind::Background,
            AssetSlot::Background,
            images.background,
        );
        received.put(
            &mut self.pages,
            AssetKind::IconBanner,
            AssetSlot::Icon,
            images.icon,
        );
        received.put(
            &mut self.pages,
            AssetKind::IconBanner,
            AssetSlot::Banner,
            images.banner,
        );

        let free_slots = self.pages.screenshots.free_screenshots();
        let mut screenshots = images.screenshots.into_iter();
        for number in free_slots {
            let Some(screenshot) = screenshots.next() else {
                break;
            };
            received.put(
                &mut self.pages,
                AssetKind::Screenshots,
                AssetSlot::Screenshot(number),
                Some(screenshot),
            );
        }
        let did_not_fit = screenshots.next().is_some();

        if let Some(page) = received.first_page() {
            self.page = page;
        }
        if did_not_fit {
            self.modals.push(Modal::Message {
                title: NOT_ENOUGH_SPACE_TITLE.to_owned(),
                text: NOT_ENOUGH_SPACE_TEXT.to_owned(),
            });
        }
        received.any()
    }

    /// Logs the entry and adds its line to the dialog's text.
    pub(super) fn note_unread_image(
        &mut self,
        path: &Path,
        entry: &str,
        error: &(dyn Error + 'static),
        unread_images: &mut Vec<String>,
    ) {
        let file = file_display_name(path);
        let context = format!("{entry} in {file} could not be decoded");
        if let Some(log_failure) = self.record_error(&context, error) {
            self.status = StatusLine::error(log_failure);
        }
        let reason = error_chain(error).join(": ");
        unread_images.push(format!(
            "{entry} in {file} could not be decoded and was left out: {reason}"
        ));
    }

    pub(super) fn show_unread_images(&mut self, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        self.modals.push(Modal::Message {
            title: UNREAD_IMAGES_TITLE.to_owned(),
            text: lines.join("\n"),
        });
    }

    /// Every problem is logged and listed in one dialog. A single empty file gets the "No assets" dialog instead.
    fn report_load_problems(&mut self, problems: Vec<FileProblem>) {
        let only_problem = problems.len() == 1;
        let mut lines = Vec::new();
        for FileProblem { path, problem } in problems {
            let context = format!("Could not load {}", path.display());
            if let Some(log_failure) = self.record_error(&context, &problem) {
                self.status = StatusLine::error(log_failure);
            }
            let file = file_display_name(&path);
            match problem {
                LoadProblem::NoAssets if only_problem => self.modals.push(Modal::Message {
                    title: NO_ASSETS_TITLE.to_owned(),
                    text: format!("{file} doesn't contain any assets."),
                }),
                problem => lines.push(format!("{file}: {problem}")),
            }
        }
        if lines.is_empty() {
            return;
        }
        let text = format!("{}\n\n{LOAD_PROBLEMS_FOOTER}", lines.join("\n"));
        self.modals.push(Modal::Message {
            title: LOAD_PROBLEMS_TITLE.to_owned(),
            text,
        });
    }
}

/// Which editor pages an FSD file put an image on.
#[derive(Default)]
struct FsdReceived {
    kinds: Vec<AssetKind>,
}

impl FsdReceived {
    fn put(
        &mut self,
        pages: &mut EditorPages,
        kind: AssetKind,
        slot: AssetSlot,
        image: Option<PreparedImage>,
    ) {
        let Some(image) = image else {
            return;
        };
        pages.get_mut(kind).set_prepared(slot, image);
        self.kinds.push(kind);
    }

    fn any(&self) -> bool {
        !self.kinds.is_empty()
    }

    /// The first page that received an image, in sidebar order (`AssetKind::ALL` follows it).
    fn first_page(&self) -> Option<Page> {
        AssetKind::ALL
            .into_iter()
            .find(|kind| self.kinds.contains(kind))
            .map(Page::of_kind)
    }
}

/// Whether a dropped file is left for the image rules: not an `.asset` or FSD file, not a folder, and readable.
/// Anything else goes to the loader, which reports what is wrong with it.
fn is_dropped_image(path: &Path) -> bool {
    let mut first_bytes = Vec::new();
    let read = File::open(path).and_then(|file| file.take(MAGIC_LEN).read_to_end(&mut first_bytes));
    read.is_ok() && detect(&first_bytes) == FileKind::Unrecognised
}
