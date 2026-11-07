use aurora_asset::{
    AssetFile, AssetKind, AssetSlot, ENTRY_COUNT, SCREENSHOT_COUNT, ScreenshotNumber, TextureHeader,
};

use super::{DecodedKind, PreparedImage, SlotPreview};

/// One page: the asset file whose bytes get saved and uploaded, and the pixels the editor shows
/// for each of its entries. The two only meet when an image is set, so a moved or compacted entry
/// keeps its stored bytes and its preview moves with it.
#[derive(Debug)]
pub struct AssetDocument {
    kind: AssetKind,
    file: AssetFile,
    previews: [SlotPreview; ENTRY_COUNT],
    revisions: [u64; ENTRY_COUNT],
}

impl AssetDocument {
    pub fn new(kind: AssetKind) -> Self {
        Self {
            kind,
            file: AssetFile::new(),
            previews: std::array::from_fn(|_| SlotPreview::Empty),
            revisions: [0; ENTRY_COUNT],
        }
    }

    pub fn kind(&self) -> AssetKind {
        self.kind
    }

    pub fn preview(&self, slot: AssetSlot) -> &SlotPreview {
        &self.previews[slot.index()]
    }

    pub fn texture_header(&self, slot: AssetSlot) -> Option<&TextureHeader> {
        self.file.get(slot).map(|texture| &texture.header)
    }

    /// Changes whenever the slot's preview or stored texture changes.
    pub fn revision(&self, slot: AssetSlot) -> u64 {
        self.revisions[slot.index()]
    }

    pub fn has_content(&self) -> bool {
        self.file.has_entries_for(self.kind)
    }

    pub fn set_prepared(&mut self, slot: AssetSlot, prepared: PreparedImage) {
        self.assert_owns(slot);
        self.file.set(slot, Some(prepared.texture));
        self.previews[slot.index()] = SlotPreview::Pixels(prepared.pixels);
        self.bump(slot);
    }

    pub fn remove(&mut self, slot: AssetSlot) {
        self.assert_owns(slot);
        self.file.set(slot, None);
        self.previews[slot.index()] = SlotPreview::Empty;
        self.bump(slot);
    }

    /// Takes over every entry of the page from a loaded file, keeping this file's offsets.
    pub fn replace_with(&mut self, decoded: DecodedKind) {
        assert_eq!(
            decoded.kind, self.kind,
            "a decoded {:?} page was given to the {:?} document",
            decoded.kind, self.kind
        );
        self.file.replace_kind(self.kind, &decoded.file);
        for slot in self.kind.slots() {
            self.previews[slot.index()] = SlotPreview::Empty;
            self.bump(slot);
        }
        for (slot, preview) in decoded.previews {
            self.previews[slot.index()] = preview;
        }
    }

    pub fn screenshot_count(&self) -> usize {
        ScreenshotNumber::ALL
            .into_iter()
            .filter(|number| self.file.get(AssetSlot::Screenshot(*number)).is_some())
            .count()
    }

    /// Empty screenshot positions in order; none on a page that holds no screenshots.
    pub fn free_screenshots(&self) -> Vec<ScreenshotNumber> {
        if self.kind != AssetKind::Screenshots {
            return Vec::new();
        }
        ScreenshotNumber::ALL
            .into_iter()
            .filter(|number| self.file.get(AssetSlot::Screenshot(*number)).is_none())
            .collect()
    }

    /// Whether the screenshots already sit in positions 1, 2, 3, ... with no gap.
    pub fn screenshots_are_compact(&self) -> bool {
        ScreenshotNumber::ALL
            .into_iter()
            .skip(self.screenshot_count())
            .all(|number| !self.holds_screenshot(number))
    }

    /// Where `number` lands when the page is compacted: a screenshot after the ones before it, an empty
    /// position on the first free one.
    pub fn compacted_position(&self, number: ScreenshotNumber) -> ScreenshotNumber {
        let index = if self.holds_screenshot(number) {
            ScreenshotNumber::ALL
                .into_iter()
                .take_while(|earlier| *earlier != number)
                .filter(|earlier| self.holds_screenshot(*earlier))
                .count()
        } else {
            self.screenshot_count()
        };
        ScreenshotNumber::ALL[index.min(SCREENSHOT_COUNT - 1)]
    }

    /// Packs the screenshots into positions 1, 2, 3, ... by moving stored bytes and previews.
    pub fn compact_screenshots(&mut self) {
        self.file.compact_screenshots();
        // A preview is Empty exactly when its slot holds no texture, so sorting the empty ones
        // last mirrors what `compact_screenshots` did to the entries.
        self.screenshot_previews()
            .sort_by_key(|preview| matches!(preview, SlotPreview::Empty));
        self.bump_all_screenshots();
    }

    /// Empties the page. Revisions keep counting up, so a view never mistakes new content for what it shows.
    pub fn clear(&mut self) {
        self.file = AssetFile::new();
        self.previews = std::array::from_fn(|_| SlotPreview::Empty);
        for revision in &mut self.revisions {
            *revision += 1;
        }
    }

    /// Drag-reorder: the screenshot at `from` lands at `to`, shifting those in between.
    pub fn move_screenshot(&mut self, from: ScreenshotNumber, to: ScreenshotNumber) {
        if self.file.get(AssetSlot::Screenshot(from)).is_none() {
            return;
        }
        self.file.move_screenshot(from, to);
        let (from, to) = (usize::from(from.get()) - 1, usize::from(to.get()) - 1);
        let previews = self.screenshot_previews();
        if from < to {
            previews[from..=to].rotate_left(1);
        } else {
            previews[to..=from].rotate_right(1);
        }
        self.bump_all_screenshots();
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        self.file.to_bytes()
    }

    fn holds_screenshot(&self, number: ScreenshotNumber) -> bool {
        self.file.get(AssetSlot::Screenshot(number)).is_some()
    }

    fn screenshot_previews(&mut self) -> &mut [SlotPreview] {
        &mut self.previews[ENTRY_COUNT - SCREENSHOT_COUNT..]
    }

    fn bump(&mut self, slot: AssetSlot) {
        self.revisions[slot.index()] += 1;
    }

    fn bump_all_screenshots(&mut self) {
        for number in ScreenshotNumber::ALL {
            self.bump(AssetSlot::Screenshot(number));
        }
    }

    fn assert_owns(&self, slot: AssetSlot) {
        assert!(
            self.kind.slots().any(|owned| owned == slot),
            "{slot:?} does not belong to the {:?} page",
            self.kind
        );
    }
}
