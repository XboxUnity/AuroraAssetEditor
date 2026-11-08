#![forbid(unsafe_code)]

mod app_harness;
mod fake_clipboard;
mod fake_dialogs;
mod fake_folder_opener;
mod fixed_clock;
mod headless_ui;
mod preview_jobs;
mod queued_runner;
mod sample_assets;
mod screenshots;

pub use app_harness::{
    AppHarness, OutcomeStage, STAGED_CONSOLE_HOST, TEST_SUBNET_PREFIX, test_build_info,
};
pub use asset_fixtures::{
    BACKGROUND, BANNER, BOXART, COVER, FsdBuilder, ICON, PREVIEW, SCREENSHOT, SLOT,
};
pub use fake_clipboard::{FakeClipboard, FakeClipboardHandle};
pub use fake_dialogs::{DialogRequest, FakeDialogs, FakeDialogsHandle};
pub use fake_folder_opener::{FakeFolderOpener, FakeFolderOpenerHandle};
pub use fixed_clock::FixedClock;
pub use headless_ui::HeadlessUi;
pub use preview_jobs::{PreviewJobs, settle_preview_spaces};
pub use queued_runner::QueuedRunner;
pub use sample_assets::{asset_file_with, placeholder};
pub use screenshots::{
    AreaReport, BASELINE_COPY_FOLDER, BASELINE_VARIABLE, Baseline, BaselineError, BaselineSource,
    CONTACT_SHEET_FILE, DEFAULT_LABEL, DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR, DIFF_FOLDER,
    DIFF_GAP_COLOUR, DIFF_PANEL_GAP, FORBIDDEN_SCREEN_TEXT, IMAGES_FOLDER, ImageChange,
    ImageCounts, ImageReport, LABEL_VARIABLE, MANIFEST_FILE, MANIFEST_FORMAT, ManifestError,
    PREVIOUS_FOLDER, PixelBounds, PixelComparison, PixelDifference, PngFileError, REPORT_FILE,
    RunFolder, RunManifest, SCREEN_THEMES, SCREENS_VARIABLE, SETTLE_TIME, Scene, Screen,
    ScreenArea, ScreenFilterOnCi, ScreenTextProblem, ScreenshotFolders, ScreenshotSuite,
    SuiteReport, TEST_ASSETS_VARIABLE, TestAssets, TestAssetsError, changed_pixels,
    check_screen_text, compare_pixels, current_platform, diff_image, image_name, read_png,
    write_contact_sheet, write_opaque_png,
};
