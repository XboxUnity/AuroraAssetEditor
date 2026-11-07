use aurora_asset::{AssetFile, AssetKind, AssetSlot, DecodeError, decode};

use super::SlotPreview;

/// An entry that could not be decoded and was left out of the loaded page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedEntry {
    pub slot: AssetSlot,
    pub error: DecodeError,
}

/// One page's entries of a loaded file, with what the editor shows for each.
#[derive(Debug, Clone)]
pub struct DecodedKind {
    pub kind: AssetKind,
    pub file: AssetFile,
    /// One item per entry still in `file`.
    pub previews: Vec<(AssetSlot, SlotPreview)>,
    pub dropped: Vec<DroppedEntry>,
}

/// An unsupported format keeps its entry; any other decode failure removes it from the page.
pub fn decode_kind(source: &AssetFile, kind: AssetKind) -> DecodedKind {
    let mut file = source.only(kind);
    let mut previews = Vec::new();
    let mut dropped = Vec::new();
    for slot in kind.slots() {
        let Some(texture) = file.get(slot) else {
            continue;
        };
        match decode(texture) {
            Ok(pixels) => previews.push((slot, SlotPreview::Pixels(pixels))),
            Err(DecodeError::UnsupportedFormat(format)) => {
                previews.push((slot, SlotPreview::Unsupported(format)));
            }
            Err(error) => {
                file.set(slot, None);
                dropped.push(DroppedEntry { slot, error });
            }
        }
    }
    DecodedKind {
        kind,
        file,
        previews,
        dropped,
    }
}
