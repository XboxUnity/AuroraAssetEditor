use std::collections::HashMap;

use slint::Image;

/// Which image of a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultImage {
    Thumbnail,
    Full,
}

/// A loaded image of one result of one search. A result's image only goes from not loaded to loaded, so
/// the key names its pixels for as long as the search is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResultImageKey {
    pub generation: u64,
    pub index: usize,
    pub image: ResultImage,
}

/// The Online page's result images, converted for Slint once, so a render caused by typing or selecting
/// reuses them. A new search drops the images of the old one.
#[derive(Default)]
pub struct OnlineImageCache {
    generation: u64,
    images: HashMap<ResultImageKey, Image>,
}

impl OnlineImageCache {
    pub fn image(&mut self, key: ResultImageKey, convert: impl FnOnce() -> Image) -> Image {
        if key.generation != self.generation {
            self.images.clear();
            self.generation = key.generation;
        }
        self.images.entry(key).or_insert_with(convert).clone()
    }
}
