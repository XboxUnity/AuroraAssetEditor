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

/// How often a copy at one size is asked for before failures at that size end the asking: a failure may be
/// passing, but a picture that always fails must not keep the worker busy.
const ATTEMPTS_PER_SIZE: u32 = 2;

/// One picture in one area: its resized copy, the size last asked for, the failures at one size, and whether
/// the last render showed it.
#[derive(Default)]
struct PreviewCopy {
    resized: Option<Image>,
    requested: Option<(u32, u32)>,
    failures: Option<SizeFailures>,
    shown: bool,
}

/// The resizes to one size that failed.
struct SizeFailures {
    size: (u32, u32),
    count: u32,
}

impl PreviewCopy {
    fn needs(&self, size: (u32, u32)) -> bool {
        let resized_size = self
            .resized
            .as_ref()
            .map(|image| (image.size().width, image.size().height));
        resized_size != Some(size) && self.requested != Some(size) && !self.gave_up_at(size)
    }

    fn gave_up_at(&self, size: (u32, u32)) -> bool {
        self.failures
            .as_ref()
            .is_some_and(|failures| failures.size == size && failures.count >= ATTEMPTS_PER_SIZE)
    }

    /// The resize to `size` failed. Counted only while `size` is the one asked for: a failure of a copy
    /// for an earlier space says nothing about the size wanted now, whose own job may still run.
    fn record_failure(&mut self, size: (u32, u32)) {
        if self.requested != Some(size) {
            log::debug!("A preview failed at {size:?}, a size no longer asked for");
            return;
        }
        self.requested = None;
        match &mut self.failures {
            Some(failures) if failures.size == size => failures.count += 1,
            _ => self.failures = Some(SizeFailures { size, count: 1 }),
        }
    }

    /// Failures count only while their size is wanted, so a size given up is asked for again once the
    /// space changed and came back, or once a copy at that size arrived.
    fn forget_failures_unless_at(&mut self, size: (u32, u32)) {
        if self
            .failures
            .as_ref()
            .is_some_and(|failures| failures.size != size)
        {
            self.failures = None;
        }
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
                copy.forget_failures_unless_at(size);
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
        let size = resized.pixels.dimensions();
        if copy.requested == Some(size) {
            copy.requested = None;
        }
        if copy
            .failures
            .as_ref()
            .is_some_and(|failures| failures.size == size)
        {
            copy.failures = None;
        }
        copy.resized = Some(slint_image(&resized.pixels));
    }

    /// The resize of `picture` for `area` to `size` failed (the failure itself is reported by the app). The
    /// next render asks for it again, once per size.
    pub fn record_failure(&mut self, picture: PictureId, area: PreviewArea, size: (u32, u32)) {
        match self.copies.get_mut(&(picture, mem::discriminant(&area))) {
            Some(copy) => copy.record_failure(size),
            None => log::debug!("A preview failed after the window stopped showing its picture"),
        }
    }

    /// Drops the copies of every picture the renders since the last call did not show.
    pub fn forget_unshown(&mut self) {
        self.copies.retain(|_, copy| mem::take(&mut copy.shown));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use aurora_asset::AssetSlot;

    use super::*;

    const PICTURE: PictureId = PictureId::Slot {
        slot: AssetSlot::Icon,
        revision: 1,
    };
    const AREA: PreviewArea = PreviewArea::Icon;
    const PIXELS: (u32, u32) = (64, 64);
    const SPACE_A: (u32, u32) = (200, 200);
    const SPACE_B: (u32, u32) = (300, 300);

    /// Keeps the jobs a `Previews` asks for.
    #[derive(Clone, Default)]
    struct Asked(Rc<RefCell<Vec<PreviewJob>>>);

    impl JobRunner for Asked {
        fn submit(&mut self, job: Job, _priority: JobPriority) {
            let Job::Preview(job) = job else {
                panic!("only preview jobs are asked for, not {job:?}");
            };
            self.0.borrow_mut().push(job);
        }
    }

    impl Asked {
        fn take(&self) -> Vec<PreviewJob> {
            self.0.borrow_mut().drain(..).collect()
        }
    }

    fn previews() -> (Previews, Asked) {
        let asked = Asked::default();
        (Previews::new(Box::new(asked.clone())), asked)
    }

    /// Settles the area at `space` and renders the picture once; returns the jobs that render asked for.
    fn render_at(previews: &mut Previews, asked: &Asked, space: (u32, u32)) -> Vec<PreviewJob> {
        previews.report_space(AREA, space);
        previews.settle_spaces();
        let pixels = RgbaImage::new(PIXELS.0, PIXELS.1);
        previews.show(PICTURE, AREA, &pixels, Image::default);
        asked.take()
    }

    fn fail(previews: &mut Previews, job: &PreviewJob) {
        previews.record_failure(job.picture, job.area, job.size());
    }

    #[test]
    fn a_size_given_up_is_asked_for_again_after_the_space_changed_and_came_back() {
        let (mut previews, asked) = previews();
        for _ in 0..ATTEMPTS_PER_SIZE {
            let jobs = render_at(&mut previews, &asked, SPACE_A);
            assert_eq!(jobs.len(), 1);
            fail(&mut previews, &jobs[0]);
        }
        assert!(
            render_at(&mut previews, &asked, SPACE_A).is_empty(),
            "given up"
        );

        let at_b = render_at(&mut previews, &asked, SPACE_B);
        assert_eq!(at_b.len(), 1, "a new size is asked for");
        previews.store(at_b.into_iter().next().expect("one job").run());

        assert_eq!(
            render_at(&mut previews, &asked, SPACE_A).len(),
            1,
            "back at the size given up, it is asked for again"
        );
    }

    #[test]
    fn a_failure_of_an_earlier_size_is_not_charged_to_the_size_asked_for_now() {
        let (mut previews, asked) = previews();
        let at_a = render_at(&mut previews, &asked, SPACE_A);
        let at_b = render_at(&mut previews, &asked, SPACE_B);
        assert_eq!((at_a.len(), at_b.len()), (1, 1));

        fail(&mut previews, &at_a[0]);
        assert!(
            render_at(&mut previews, &asked, SPACE_B).is_empty(),
            "the job for the size wanted now still runs, so nothing is asked twice"
        );

        fail(&mut previews, &at_b[0]);
        assert_eq!(
            render_at(&mut previews, &asked, SPACE_B).len(),
            1,
            "its own first failure is retried"
        );
    }
}
