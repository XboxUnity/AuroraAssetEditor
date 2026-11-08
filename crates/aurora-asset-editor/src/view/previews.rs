use std::collections::HashMap;
use std::mem::{self, Discriminant};

use aurora_asset::RgbaImage;
use aurora_image::preview_size;
use aurora_ui::PreviewArea;
use slint::Image;

use crate::jobs::{Job, JobPriority, JobRunner, PreviewJob, ResizedPreview};

use super::{PictureId, slint_image};

/// Every picture the window shows, resized to the space its area gives it (the software renderer would scale it
/// nearest-neighbour). The resizing runs as preview jobs; until a picture's copy for the current space arrives,
/// the window keeps showing what it had: the copy for an earlier space, else the picture as it is.
///
/// A new space counts once it is settled: `report_space` records it, `settle_spaces` puts it to use, so the
/// window decides how long a space must hold still before every picture in it is resized again.
pub struct Previews {
    jobs: Box<dyn JobRunner>,
    settled: HashMap<AreaKey, (u32, u32)>,
    reported: HashMap<AreaKey, (u32, u32)>,
    copies: HashMap<(PictureId, AreaKey), PreviewCopy>,
}

/// An area as a map key: Slint's enums cannot be hashed, their variants can.
type AreaKey = Discriminant<PreviewArea>;

/// One picture in one area: its resized copy, the size last asked for, and whether the last render showed it.
#[derive(Default)]
struct PreviewCopy {
    resized: Option<Image>,
    requested: Option<(u32, u32)>,
    shown: bool,
}

impl PreviewCopy {
    fn needs(&self, size: (u32, u32)) -> bool {
        let resized_size = self
            .resized
            .as_ref()
            .map(|image| (image.size().width, image.size().height));
        resized_size != Some(size) && self.requested != Some(size)
    }
}

/// A picture as the window gets it. Compared by which picture it is and its size, never by its pixels.
#[derive(Debug, Clone)]
pub struct ShownPicture {
    pub id: PictureId,
    pub image: Image,
}

impl PartialEq for ShownPicture {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.image.size() == other.image.size()
    }
}

impl Previews {
    pub fn new(jobs: Box<dyn JobRunner>) -> Previews {
        Previews {
            jobs,
            settled: HashMap::new(),
            reported: HashMap::new(),
            copies: HashMap::new(),
        }
    }

    /// Records the space `area` gives its pictures, in physical pixels; `true` when it differs from the space
    /// already recorded, so it needs settling.
    pub fn report_space(&mut self, area: PreviewArea, space: (u32, u32)) -> bool {
        let area = mem::discriminant(&area);
        let recorded = self.reported.get(&area).or(self.settled.get(&area));
        if recorded == Some(&space) {
            return false;
        }
        self.reported.insert(area, space);
        true
    }

    /// Puts the reported spaces to use; the next render asks for the copies they need.
    pub fn settle_spaces(&mut self) {
        self.settled.extend(self.reported.drain());
    }

    /// The picture `id` as `area` shows it, asking for a copy at the area's settled space when it has none.
    /// `original` gives the picture as it is.
    pub fn show(
        &mut self,
        id: PictureId,
        area: PreviewArea,
        pixels: &RgbaImage,
        original: impl FnOnce() -> Image,
    ) -> ShownPicture {
        let wanted = self
            .settled
            .get(&mem::discriminant(&area))
            .map(|space| (*space, preview_size(pixels.dimensions(), *space)));
        let copy = self
            .copies
            .entry((id, mem::discriminant(&area)))
            .or_default();
        copy.shown = true;
        let image = match wanted {
            Some((_, size)) if size == pixels.dimensions() => original(),
            Some((space, size)) => {
                if copy.needs(size) {
                    let job = PreviewJob {
                        picture: id,
                        area,
                        space,
                        pixels: pixels.clone(),
                    };
                    self.jobs.submit(Job::Preview(job), JobPriority::Background);
                    copy.requested = Some(size);
                }
                copy.resized.clone().unwrap_or_else(original)
            }
            None => copy.resized.clone().unwrap_or_else(original),
        };
        ShownPicture { id, image }
    }

    /// Keeps a finished copy for the next render.
    pub fn store(&mut self, resized: ResizedPreview) {
        let key = (resized.picture, mem::discriminant(&resized.area));
        let Some(copy) = self.copies.get_mut(&key) else {
            log::debug!("A preview was resized after the window stopped showing its picture");
            return;
        };
        if copy.requested == Some(resized.pixels.dimensions()) {
            copy.requested = None;
        }
        copy.resized = Some(slint_image(&resized.pixels));
    }

    /// Drops the copies of every picture the renders since the last call did not show.
    pub fn forget_unshown(&mut self) {
        self.copies.retain(|_, copy| mem::take(&mut copy.shown));
    }
}
