mod baseline;
mod capture_thread;
mod contact_sheet;
mod diff_image;
mod image_change;
mod image_files;
mod path_error;
mod pixel_comparison;
mod png_file;
mod run_folder;
mod run_manifest;
mod scene;
mod screen;
mod screen_export;
mod screen_text_check;
mod screenshot_folders;
mod screenshot_suite;
mod suite_report;
mod test_assets;

pub use baseline::{Baseline, BaselineError};
pub use contact_sheet::{CONTACT_SHEET_FILE, write_contact_sheet};
pub use diff_image::{
    DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR, DIFF_GAP_COLOUR, DIFF_PANEL_GAP, diff_image,
};
pub use image_change::ImageChange;
pub use image_files::is_kebab_case;
pub use pixel_comparison::{
    PixelBounds, PixelComparison, PixelDifference, changed_pixels, compare_pixels,
};
pub use png_file::{PngFileError, read_png, write_opaque_png};
pub use run_folder::{
    BASELINE_COPY_FOLDER, DIFF_FOLDER, IMAGES_FOLDER, RunFolder, current_platform,
};
pub use run_manifest::{MANIFEST_FILE, MANIFEST_FORMAT, ManifestError, RunManifest};
pub use scene::{SETTLE_TIME, Scene};
pub use screen::{SCREEN_THEMES, Screen, ScreenArea, image_name};
pub use screen_export::{ExportReport, ScreenExport};
pub use screen_text_check::{FORBIDDEN_SCREEN_TEXT, ScreenTextProblem, check_screen_text};
pub use screenshot_folders::{PREVIOUS_FOLDER, ScreenshotFolders};
pub use screenshot_suite::{
    BASELINE_VARIABLE, BaselineSource, DEFAULT_LABEL, LABEL_VARIABLE, REPORT_FILE,
    SCREENS_VARIABLE, ScreenFilterOnCi, ScreenshotSuite,
};
pub use suite_report::{AreaReport, ImageCounts, ImageReport, SuiteReport};
pub use test_assets::{TEST_ASSETS_VARIABLE, TestAssets, TestAssetsError};
