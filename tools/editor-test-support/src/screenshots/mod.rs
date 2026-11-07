mod diff_image;
mod pixel_comparison;
mod png_file;
mod reference_mode;
mod reference_store;
mod scene;
mod screen_text_check;

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
pub use screen_text_check::{FORBIDDEN_SCREEN_TEXT, ScreenTextProblem, check_screen_text};
