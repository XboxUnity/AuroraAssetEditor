use std::collections::HashMap;

use aurora_asset::AssetSlot;
use slint::Image;

/// Each slot's image as the window last received it, with the revision it was made from, so a render
/// only converts the images whose revision changed.
#[derive(Debug, Default)]
pub struct ImageCache {
    shown: HashMap<AssetSlot, (u64, Image)>,
}

impl ImageCache {
    /// The image for `revision` of `slot`; `convert` runs only when the cache holds another revision.
    pub fn image(
        &mut self,
        slot: AssetSlot,
        revision: u64,
        convert: impl FnOnce() -> Image,
    ) -> Image {
        match self.shown.get(&slot) {
            Some((shown_revision, image)) if *shown_revision == revision => image.clone(),
            _ => {
                let image = convert();
                self.shown.insert(slot, (revision, image.clone()));
                image
            }
        }
    }
}
