use aurora_asset::AssetSlot;

use super::ResultImageKey;

/// One picture the window shows, as the pixels it is made from: a new revision of a slot or a new search
/// is another picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureId {
    Slot { slot: AssetSlot, revision: u64 },
    Result(ResultImageKey),
}
