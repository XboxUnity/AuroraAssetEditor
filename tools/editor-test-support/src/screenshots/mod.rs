mod contact_sheet;
mod diff_image;
mod pixel_comparison;
mod png_file;
mod reference_mode;
mod reference_store;
mod scene;
mod screen;
mod screen_text_check;
mod screenshot_suite;
mod suite_report;
mod test_assets;

pub use contact_sheet::{CONTACT_SHEET_FILE, write_contact_sheet};
pub use diff_image::{
    DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR, DIFF_GAP_COLOUR, DIFF_PANEL_GAP, diff_image,
};
pub use pixel_comparison::{
    PixelBounds, PixelComparison, PixelDifference, changed_pixels, compare_pixels,
};
pub use png_file::{PngFileError, read_png, write_opaque_png};
pub use reference_mode::{CI_VARIABLE, REFERENCES_VARIABLE, ReferenceMode, ReferenceModeError};
pub use reference_store::{ImageStatus, ReferenceStore};
pub use scene::{SETTLE_TIME, Scene};
pub use screen::{SCREEN_THEMES, Screen, ScreenArea, image_name};
pub use screen_text_check::{FORBIDDEN_SCREEN_TEXT, ScreenTextProblem, check_screen_text};
pub use screenshot_suite::{REPORT_FILE, SCREENS_VARIABLE, ScreenFilterOnCi, ScreenshotSuite};
pub use suite_report::{AreaReport, ImageCounts, ImageReport, SuiteReport};
pub use test_assets::{TEST_ASSETS_VARIABLE, TestAssets, TestAssetsError};
