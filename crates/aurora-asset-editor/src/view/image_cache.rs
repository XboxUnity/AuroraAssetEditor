use std::collections::HashMap;

use aurora_asset::AssetSlot;

/// The revision of each slot's image as the window last received it, so a render only converts and
/// reassigns the images that changed.
#[derive(Debug, Default)]
pub struct ImageCache {
    shown_revisions: HashMap<AssetSlot, u64>,
}

impl ImageCache {
    /// `true` when the window does not show `revision` of `slot` yet; it is then recorded as shown.
    pub fn needs_update(&mut self, slot: AssetSlot, revision: u64) -> bool {
        let previous = self.shown_revisions.insert(slot, revision);
        previous != Some(revision)
    }
}
