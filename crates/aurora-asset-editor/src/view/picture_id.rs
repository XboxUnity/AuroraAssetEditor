use aurora_asset::AssetSlot;

use super::{Placeholder, ResultImageKey};

/// One picture the window shows, as the pixels it is made from: a new revision of a slot or a new search
/// is another picture. The placeholder art of an empty slot never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureId {
    Slot { slot: AssetSlot, revision: u64 },
    Result(ResultImageKey),
    Placeholder(Placeholder),
}
