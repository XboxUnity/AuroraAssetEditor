use aurora_asset::{AssetKind, AssetSlot};

use super::AssetDocument;

/// The four pages the editor holds.
#[derive(Debug)]
pub struct EditorPages {
    pub cover: AssetDocument,
    pub background: AssetDocument,
    pub icon_banner: AssetDocument,
    pub screenshots: AssetDocument,
}

impl Default for EditorPages {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPages {
    pub fn new() -> Self {
        Self {
            cover: AssetDocument::new(AssetKind::Cover),
            background: AssetDocument::new(AssetKind::Background),
            icon_banner: AssetDocument::new(AssetKind::IconBanner),
            screenshots: AssetDocument::new(AssetKind::Screenshots),
        }
    }

    pub fn get(&self, kind: AssetKind) -> &AssetDocument {
        match kind {
            AssetKind::Cover => &self.cover,
            AssetKind::Background => &self.background,
            AssetKind::IconBanner => &self.icon_banner,
            AssetKind::Screenshots => &self.screenshots,
        }
    }

    pub fn get_mut(&mut self, kind: AssetKind) -> &mut AssetDocument {
        match kind {
            AssetKind::Cover => &mut self.cover,
            AssetKind::Background => &mut self.background,
            AssetKind::IconBanner => &mut self.icon_banner,
            AssetKind::Screenshots => &mut self.screenshots,
        }
    }

    /// The page that owns `slot`; `None` for the unused fourth entry.
    pub fn document_for_mut(&mut self, slot: AssetSlot) -> Option<&mut AssetDocument> {
        let kind = AssetKind::ALL
            .into_iter()
            .find(|kind| kind.slots().any(|owned| owned == slot))?;
        Some(self.get_mut(kind))
    }

    pub fn reset(&mut self) {
        for kind in AssetKind::ALL {
            self.get_mut(kind).clear();
        }
    }

    /// The pages among `kinds` that changed since they were last opened or saved, in sidebar order.
    pub fn unsaved_among(&self, kinds: &[AssetKind]) -> Vec<AssetKind> {
        AssetKind::ALL
            .into_iter()
            .filter(|kind| kinds.contains(kind) && self.get(*kind).has_unsaved_changes())
            .collect()
    }

    /// How many of cover, background, icon, banner and screenshots hold something (0 to 5).
    pub fn present_types(&self) -> usize {
        [
            self.cover.has_content(),
            self.background.has_content(),
            self.has_slot(AssetSlot::Icon),
            self.has_slot(AssetSlot::Banner),
            self.screenshots.has_content(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
    }

    fn has_slot(&self, slot: AssetSlot) -> bool {
        self.icon_banner.texture_header(slot).is_some()
    }
}
