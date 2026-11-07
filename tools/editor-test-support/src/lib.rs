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
    DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR, DIFF_GAP_COLOUR, DIFF_PANEL_GAP, PixelBounds,
    PixelComparison, PixelDifference, PngFileError, changed_pixels, compare_pixels, diff_image,
    read_png, write_opaque_png,
};
