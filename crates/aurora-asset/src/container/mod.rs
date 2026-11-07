mod asset_file;
mod asset_kind;
mod asset_slot;
mod screenshot_number;

pub use asset_file::{AssetError, AssetFile};
pub use asset_kind::AssetKind;
pub use asset_slot::AssetSlot;
pub use screenshot_number::ScreenshotNumber;

pub const ENTRY_COUNT: usize = 25;
const FIRST_SCREENSHOT_INDEX: usize = 5;
pub const SCREENSHOT_COUNT: usize = ENTRY_COUNT - FIRST_SCREENSHOT_INDEX;
