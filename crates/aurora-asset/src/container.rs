use std::fmt;

use crate::texture::{EncodedTexture, TEXTURE_HEADER_LEN, TextureHeader};

const MAGIC: u32 = 0x5258_4541;
const VERSION: u32 = 1;
const ENTRY_COUNT: usize = 25;
const FIRST_SCREENSHOT_INDEX: usize = 5;
const SCREENSHOT_COUNT: usize = ENTRY_COUNT - FIRST_SCREENSHOT_INDEX;
const ENTRY_TABLE_START: usize = 20;
const ENTRY_FIXED_FIELDS_LEN: usize = 12;
const ENTRY_LEN: usize = ENTRY_FIXED_FIELDS_LEN + TEXTURE_HEADER_LEN;
const DATA_START: usize = 2048;

/// A screenshot position, 1 to 20, as the app numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenshotNumber(u8);

impl ScreenshotNumber {
    pub fn new(number: u8) -> Option<Self> {
        if (1..=SCREENSHOT_COUNT as u8).contains(&number) {
            Some(Self(number))
        } else {
            None
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }

    fn position(self) -> usize {
        usize::from(self.0) - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetSlot {
    Icon,
    Banner,
    Cover,
    Slot,
    Background,
    Screenshot(ScreenshotNumber),
}

impl AssetSlot {
    /// The entry number in the file; with `from_index`, the only place the numbering lives.
    pub fn index(self) -> usize {
        match self {
            Self::Icon => 0,
            Self::Banner => 1,
            Self::Cover => 2,
            Self::Slot => 3,
            Self::Background => 4,
            Self::Screenshot(number) => FIRST_SCREENSHOT_INDEX + number.position(),
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Icon),
            1 => Some(Self::Banner),
            2 => Some(Self::Cover),
            3 => Some(Self::Slot),
            4 => Some(Self::Background),
            FIRST_SCREENSHOT_INDEX..ENTRY_COUNT => {
                let number = (index - FIRST_SCREENSHOT_INDEX + 1) as u8;
                ScreenshotNumber::new(number).map(Self::Screenshot)
            }
            _ => None,
        }
    }

    pub fn screenshot(number: u8) -> Option<Self> {
        ScreenshotNumber::new(number).map(Self::Screenshot)
    }
}

/// The page of the app that owns a group of entries (GC, BK, GL, SS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Cover,
    Background,
    IconBanner,
    Screenshots,
}

impl AssetKind {
    fn owns(self, slot: AssetSlot) -> bool {
        match self {
            Self::Cover => slot == AssetSlot::Cover,
            Self::Background => slot == AssetSlot::Background,
            Self::IconBanner => matches!(slot, AssetSlot::Icon | AssetSlot::Banner),
            Self::Screenshots => matches!(slot, AssetSlot::Screenshot(_)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    TooShort {
        len: usize,
    },
    BadMagic(u32),
    UnsupportedVersion(u32),
    EntryOutOfBounds {
        slot: AssetSlot,
        end: usize,
        len: usize,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(
                f,
                "asset file is {len} bytes, shorter than the {DATA_START}-byte header"
            ),
            Self::BadMagic(magic) => write!(f, "asset file magic {magic:#010x} is not RXEA"),
            Self::UnsupportedVersion(version) => {
                write!(f, "asset file version {version} is not supported")
            }
            Self::EntryOutOfBounds { slot, end, len } => write!(
                f,
                "asset entry {} ends at byte {end}, past the end of the {len}-byte file",
                slot.index()
            ),
        }
    }
}

impl std::error::Error for AssetError {}

/// An Aurora `.asset` file: 25 optional textures, written the way the old app wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFile {
    entries: [Option<EncodedTexture>; ENTRY_COUNT],
}

impl Default for AssetFile {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetFile {
    pub fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| None),
        }
    }

    /// Stored offsets and the data size are ignored: data is read back to back in entry order.
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        if bytes.len() < DATA_START {
            return Err(AssetError::TooShort { len: bytes.len() });
        }
        let magic = read_u32(bytes, 0);
        if magic != MAGIC {
            return Err(AssetError::BadMagic(magic));
        }
        let version = read_u32(bytes, 4);
        if version != VERSION {
            return Err(AssetError::UnsupportedVersion(version));
        }

        let mut file = Self::new();
        let mut data_cursor = DATA_START;
        for (index, entry) in file.entries.iter_mut().enumerate() {
            let entry_start = ENTRY_TABLE_START + index * ENTRY_LEN;
            let size = read_u32(bytes, entry_start + 4) as usize;
            if size == 0 {
                continue;
            }
            let end = data_cursor.saturating_add(size);
            let Some(data) = bytes.get(data_cursor..end) else {
                return Err(AssetError::EntryOutOfBounds {
                    slot: slot_at(index),
                    end,
                    len: bytes.len(),
                });
            };
            let header_start = entry_start + ENTRY_FIXED_FIELDS_LEN;
            let mut header = [0; TEXTURE_HEADER_LEN];
            header.copy_from_slice(&bytes[header_start..header_start + TEXTURE_HEADER_LEN]);
            *entry = Some(EncodedTexture {
                header: TextureHeader::from_bytes(header),
                data: data.to_vec(),
            });
            data_cursor = end;
        }
        Ok(file)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let data_size: usize = self.present().map(|texture| texture.data.len()).sum();
        let mut out = Vec::with_capacity(DATA_START + data_size);
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.extend_from_slice(&VERSION.to_be_bytes());
        out.extend_from_slice(&(data_size as u32).to_be_bytes());
        out.extend_from_slice(&self.flags().to_be_bytes());
        out.extend_from_slice(&self.screenshot_count().to_be_bytes());

        let mut next_offset = 0u32;
        for entry in &self.entries {
            let (offset, size, header) = match entry {
                Some(texture) => {
                    let size = texture.data.len() as u32;
                    let offset = next_offset;
                    next_offset = next_offset.wrapping_add(size);
                    (offset, size, texture.header.to_bytes())
                }
                None => (0, 0, [0; TEXTURE_HEADER_LEN]),
            };
            out.extend_from_slice(&offset.to_be_bytes());
            out.extend_from_slice(&size.to_be_bytes());
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&header);
        }
        out.resize(DATA_START, 0);
        for texture in self.present() {
            out.extend_from_slice(&texture.data);
        }
        out
    }

    pub fn get(&self, slot: AssetSlot) -> Option<&EncodedTexture> {
        self.entries[slot.index()].as_ref()
    }

    pub fn set(&mut self, slot: AssetSlot, texture: Option<EncodedTexture>) {
        self.entries[slot.index()] = texture;
    }

    pub fn primary_kind(&self) -> Option<AssetKind> {
        [
            AssetKind::Cover,
            AssetKind::Background,
            AssetKind::Screenshots,
            AssetKind::IconBanner,
        ]
        .into_iter()
        .find(|kind| self.has_entries_for(*kind))
    }

    /// A copy holding only the entries that page owns; the Slot entry is never kept.
    pub fn only(&self, kind: AssetKind) -> AssetFile {
        let mut kept = self.clone();
        for (index, entry) in kept.entries.iter_mut().enumerate() {
            if !kind.owns(slot_at(index)) {
                *entry = None;
            }
        }
        kept
    }

    /// Packs the screenshots into slots 1, 2, 3, ... keeping their order and stored bytes.
    pub fn compact_screenshots(&mut self) {
        let screenshots = &mut self.entries[FIRST_SCREENSHOT_INDEX..];
        let mut next_free = 0;
        for position in 0..screenshots.len() {
            if screenshots[position].is_some() {
                screenshots.swap(next_free, position);
                next_free += 1;
            }
        }
    }

    /// Drag-reorder: the screenshot leaves `from` and lands at `to`, shifting those in between.
    /// An empty `from` has nothing to move and changes nothing.
    pub fn move_screenshot(&mut self, from: ScreenshotNumber, to: ScreenshotNumber) {
        let screenshots = &mut self.entries[FIRST_SCREENSHOT_INDEX..];
        let (from, to) = (from.position(), to.position());
        if screenshots[from].is_none() {
            return;
        }
        if from < to {
            screenshots[from..=to].rotate_left(1);
        } else {
            screenshots[to..=from].rotate_right(1);
        }
    }

    fn present(&self) -> impl Iterator<Item = &EncodedTexture> {
        self.entries.iter().flatten()
    }

    fn flags(&self) -> u32 {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_some())
            .fold(0, |flags, (index, _)| flags | 1 << index)
    }

    fn screenshot_count(&self) -> u32 {
        self.entries[FIRST_SCREENSHOT_INDEX..]
            .iter()
            .filter(|entry| entry.is_some())
            .count() as u32
    }

    fn has_entries_for(&self, kind: AssetKind) -> bool {
        self.entries
            .iter()
            .enumerate()
            .any(|(index, entry)| entry.is_some() && kind.owns(slot_at(index)))
    }
}

/// Entry indices below `ENTRY_COUNT` always map to a slot.
fn slot_at(index: usize) -> AssetSlot {
    AssetSlot::from_index(index).unwrap_or(AssetSlot::Slot)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut word = [0; 4];
    word.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_be_bytes(word)
}
