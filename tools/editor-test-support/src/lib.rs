#![forbid(unsafe_code)]

mod app_harness;
mod fake_clipboard;
mod fake_dialogs;
mod fake_folder_opener;
mod fixed_clock;
mod headless_ui;
mod queued_runner;
mod sample_assets;
mod screenshots;

pub use app_harness::{AppHarness, OutcomeStage, TEST_SUBNET_PREFIX, test_build_info};
pub use asset_fixtures::{
    BACKGROUND, BANNER, BOXART, COVER, FsdBuilder, ICON, PREVIEW, SCREENSHOT, SLOT,
};
pub use fake_clipboard::{FakeClipboard, FakeClipboardHandle};
pub use fake_dialogs::{DialogRequest, FakeDialogs, FakeDialogsHandle};
pub use fake_folder_opener::{FakeFolderOpener, FakeFolderOpenerHandle};
pub use fixed_clock::FixedClock;
pub use headless_ui::HeadlessUi;
pub use queued_runner::QueuedRunner;
pub use sample_assets::{asset_file_with, placeholder};
pub use screenshots::{
    AreaReport, CI_VARIABLE, CONTACT_SHEET_FILE, DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR,
    DIFF_GAP_COLOUR, DIFF_PANEL_GAP, FORBIDDEN_SCREEN_TEXT, ImageCounts, ImageReport, ImageStatus,
    PixelBounds, PixelComparison, PixelDifference, PngFileError, REFERENCES_VARIABLE, REPORT_FILE,
    ReferenceMode, ReferenceModeError, ReferenceStore, SCREEN_THEMES, SCREENS_VARIABLE,
    SETTLE_TIME, Scene, Screen, ScreenArea, ScreenTextProblem, ScreenshotSuite, SuiteReport,
    changed_pixels, check_screen_text, compare_pixels, diff_image, image_name, read_png,
    write_contact_sheet, write_opaque_png,
};
