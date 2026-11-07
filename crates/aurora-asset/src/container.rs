use std::fmt;

use crate::bytes::read_u32_be;
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
/// Always written as 0: the field held a pointer in the console memory.
const EXTENDED_INFO: u32 = 0;

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
    pub const ALL: [AssetKind; 4] = [
        Self::Cover,
        Self::Background,
        Self::IconBanner,
        Self::Screenshots,
    ];

    /// The two letters Aurora puts in front of the title ID in an asset file name.
    pub fn file_prefix(self) -> &'static str {
        match self {
            Self::Cover => "GC",
            Self::Background => "BK",
            Self::IconBanner => "GL",
            Self::Screenshots => "SS",
        }
    }

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

/// One of the 25 table entries. Like the old app, an empty entry still carries the offset and
/// header it had when loaded; Aurora files in the wild contain such residue.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    offset: u32,
    texture: Option<EncodedTexture>,
    empty_header: [u8; TEXTURE_HEADER_LEN],
}

impl Entry {
    fn empty() -> Self {
        Self {
            offset: 0,
            texture: None,
            empty_header: [0; TEXTURE_HEADER_LEN],
        }
    }

    fn header_bytes(&self) -> [u8; TEXTURE_HEADER_LEN] {
        match &self.texture {
            Some(texture) => texture.header.to_bytes(),
            None => self.empty_header,
        }
    }
}

/// An Aurora `.asset` file: 25 optional textures, written the way the old app wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFile {
    entries: [Entry; ENTRY_COUNT],
}

impl Default for AssetFile {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetFile {
    pub fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| Entry::empty()),
        }
    }

    /// Stored offsets and the data size are ignored for data: it is read back to back in entry
    /// order. The offset and header of empty entries are kept so they are written back unchanged.
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        if bytes.len() < DATA_START {
            return Err(AssetError::TooShort { len: bytes.len() });
        }
        let magic = read_u32_be(bytes, 0);
        if magic != MAGIC {
            return Err(AssetError::BadMagic(magic));
        }
        let version = read_u32_be(bytes, 4);
        if version != VERSION {
            return Err(AssetError::UnsupportedVersion(version));
        }

        let mut file = Self::new();
        let mut data_cursor = DATA_START;
        for slot in all_slots() {
            let entry_start = ENTRY_TABLE_START + slot.index() * ENTRY_LEN;
            let size = read_u32_be(bytes, entry_start + 4) as usize;
            let header_start = entry_start + ENTRY_FIXED_FIELDS_LEN;
            let mut header = [0; TEXTURE_HEADER_LEN];
            header.copy_from_slice(&bytes[header_start..header_start + TEXTURE_HEADER_LEN]);

            let entry = &mut file.entries[slot.index()];
            entry.offset = read_u32_be(bytes, entry_start);
            if size == 0 {
                entry.empty_header = header;
                continue;
            }
            let end = data_cursor.saturating_add(size);
            let Some(data) = bytes.get(data_cursor..end) else {
                return Err(AssetError::EntryOutOfBounds {
                    slot,
                    end,
                    len: bytes.len(),
                });
            };
            entry.texture = Some(EncodedTexture {
                header: TextureHeader::from_bytes(header),
                data: data.to_vec(),
            });
            data_cursor = end;
        }
        Ok(file)
    }

    /// Takes `&mut self` because the old app's save assigned the new offsets to its entries, so a
    /// texture removed afterwards keeps the offset of the last save.
    pub fn to_bytes(&mut self) -> Vec<u8> {
        self.record_offsets();
        let data_size: usize = self.present().map(|texture| texture.data.len()).sum();
        let mut out = Vec::with_capacity(DATA_START + data_size);
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.extend_from_slice(&VERSION.to_be_bytes());
        // The u32 arithmetic wraps like the old app's unchecked C# arithmetic.
        out.extend_from_slice(&(data_size as u32).to_be_bytes());
        out.extend_from_slice(&self.flags().to_be_bytes());
        out.extend_from_slice(&self.screenshot_count().to_be_bytes());

        for entry in &self.entries {
            let size = entry
                .texture
                .as_ref()
                .map_or(0, |texture| texture.data.len());
            out.extend_from_slice(&entry.offset.to_be_bytes());
            out.extend_from_slice(&(size as u32).to_be_bytes());
            out.extend_from_slice(&EXTENDED_INFO.to_be_bytes());
            out.extend_from_slice(&entry.header_bytes());
        }
        out.resize(DATA_START, 0);
        for texture in self.present() {
            out.extend_from_slice(&texture.data);
        }
        out
    }

    pub fn get(&self, slot: AssetSlot) -> Option<&EncodedTexture> {
        self.entries[slot.index()].texture.as_ref()
    }

    /// Setting `None` zeroes the header and keeps the offset, as the old app did.
    pub fn set(&mut self, slot: AssetSlot, texture: Option<EncodedTexture>) {
        let entry = &mut self.entries[slot.index()];
        entry.texture = texture;
        entry.empty_header = [0; TEXTURE_HEADER_LEN];
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

    /// A fresh file (offsets 0, zero headers) with the entries that page owns copied in; the Slot
    /// entry is never kept.
    pub fn only(&self, kind: AssetKind) -> AssetFile {
        let mut kept = Self::new();
        for slot in all_slots().filter(|slot| kind.owns(*slot)) {
            let source = &self.entries[slot.index()];
            let target = &mut kept.entries[slot.index()];
            target.texture = source.texture.clone();
            target.empty_header = source.empty_header;
        }
        kept
    }

    /// Packs the screenshots into slots 1, 2, 3, ... keeping their order and stored bytes.
    pub fn compact_screenshots(&mut self) {
        self.rearrange_screenshots(|textures| {
            let mut next_free = 0;
            for position in 0..textures.len() {
                if textures[position].is_some() {
                    textures.swap(next_free, position);
                    next_free += 1;
                }
            }
        });
    }

    /// Drag-reorder: the screenshot leaves `from` and lands at `to`, shifting those in between.
    /// An empty `from` has nothing to move and changes nothing.
    pub fn move_screenshot(&mut self, from: ScreenshotNumber, to: ScreenshotNumber) {
        let (from, to) = (from.position(), to.position());
        if self.entries[FIRST_SCREENSHOT_INDEX + from]
            .texture
            .is_none()
        {
            return;
        }
        self.rearrange_screenshots(|textures| {
            if from < to {
                textures[from..=to].rotate_left(1);
            } else {
                textures[to..=from].rotate_right(1);
            }
        });
    }

    /// Moves only the textures between screenshot slots. Each slot keeps its own offset, and a
    /// slot left empty gets a zero header, as the old app's `SetScreenshot(null)` did.
    fn rearrange_screenshots(&mut self, rearrange: impl FnOnce(&mut [Option<EncodedTexture>])) {
        let screenshots = &mut self.entries[FIRST_SCREENSHOT_INDEX..];
        let mut textures: Vec<Option<EncodedTexture>> = screenshots
            .iter_mut()
            .map(|entry| entry.texture.take())
            .collect();
        rearrange(&mut textures);
        for (entry, texture) in screenshots.iter_mut().zip(textures) {
            entry.texture = texture;
            entry.empty_header = [0; TEXTURE_HEADER_LEN];
        }
    }

    /// Present entries get consecutive offsets in entry order; empty ones keep what they had.
    fn record_offsets(&mut self) {
        let mut next_offset = 0u32;
        for entry in &mut self.entries {
            if let Some(texture) = &entry.texture {
                entry.offset = next_offset;
                next_offset = next_offset.wrapping_add(texture.data.len() as u32);
            }
        }
    }

    fn present(&self) -> impl Iterator<Item = &EncodedTexture> {
        self.entries
            .iter()
            .filter_map(|entry| entry.texture.as_ref())
    }

    fn flags(&self) -> u32 {
        all_slots()
            .filter(|slot| self.get(*slot).is_some())
            .fold(0, |flags, slot| flags | 1 << slot.index())
    }

    fn screenshot_count(&self) -> u32 {
        self.entries[FIRST_SCREENSHOT_INDEX..]
            .iter()
            .filter(|entry| entry.texture.is_some())
            .count() as u32
    }

    fn has_entries_for(&self, kind: AssetKind) -> bool {
        all_slots().any(|slot| kind.owns(slot) && self.get(slot).is_some())
    }
}

fn all_slots() -> impl Iterator<Item = AssetSlot> {
    (0..ENTRY_COUNT).filter_map(AssetSlot::from_index)
}
