//! The FSD asset container (FreeStyle Dash), read for import only. All fields are big-endian.

use std::fmt;

const MAGIC: u32 = 0x4653_4441;
const VERSION: u32 = 1;
const MIN_LEN: usize = 0x1F8;

const ENTRY_COUNT: usize = 10;
const ENTRY_TABLE_OFFSET: usize = 24;
const SCREENSHOT_ENTRY_COUNT: usize = 20;
const SCREENSHOT_TABLE_OFFSET: usize = 184;
const TABLE_ROW_LEN: usize = 16;

const TYPE_ICON: u32 = 0x01;
const TYPE_BACKGROUND: u32 = 0x02;
const TYPE_BANNER: u32 = 0x04;
const TYPE_SCREENSHOT: u32 = 0x20;
const TYPE_COVER: u32 = 0x80;

const DDS_MAGIC: &[u8; 4] = b"DDS ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsdImage {
    Dds(Vec<u8>),
    /// PNG, JPEG or another format the app decodes.
    Encoded(Vec<u8>),
}

impl FsdImage {
    fn classify(data: &[u8]) -> Self {
        if data.starts_with(DDS_MAGIC) {
            Self::Dds(data.to_vec())
        } else {
            Self::Encoded(data.to_vec())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsdError {
    TooShort {
        len: usize,
    },
    BadMagic(u32),
    UnsupportedVersion(u32),
    /// `index` counts the 10 asset entries first, then the 20 screenshot entries.
    EntryOutOfBounds {
        index: usize,
    },
}

impl fmt::Display for FsdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(
                formatter,
                "the FSD file is {len} bytes, shorter than its {MIN_LEN}-byte header"
            ),
            Self::BadMagic(magic) => write!(formatter, "the FSD magic {magic:#010x} is wrong"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "FSD version {version} is not supported")
            }
            Self::EntryOutOfBounds { index } => {
                write!(
                    formatter,
                    "FSD entry {index} points past the end of the file"
                )
            }
        }
    }
}

impl std::error::Error for FsdError {}

/// An asset entry's type and data; the data is `None` when the entry is empty.
struct Entry<'a> {
    asset_type: u32,
    data: Option<&'a [u8]>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsdFile {
    pub cover: Option<FsdImage>,
    pub background: Option<FsdImage>,
    pub icon: Option<FsdImage>,
    pub banner: Option<FsdImage>,
    pub screenshots: Vec<FsdImage>,
}

impl FsdFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, FsdError> {
        if bytes.len() < MIN_LEN {
            return Err(FsdError::TooShort { len: bytes.len() });
        }
        let magic = read_u32(bytes, 0);
        if magic != MAGIC {
            return Err(FsdError::BadMagic(magic));
        }
        let version = read_u32(bytes, 4);
        if version != VERSION {
            return Err(FsdError::UnsupportedVersion(version));
        }

        let entries = (0..ENTRY_COUNT)
            .map(|index| asset_entry(bytes, index))
            .collect::<Result<Vec<_>, _>>()?;
        let table_screenshots = (0..SCREENSHOT_ENTRY_COUNT)
            .map(|index| screenshot_entry(bytes, index))
            .collect::<Result<Vec<_>, _>>()?;

        let first_of_type = |asset_type: u32| {
            entries
                .iter()
                .filter(|entry| entry.asset_type == asset_type)
                .find_map(|entry| entry.data)
                .map(FsdImage::classify)
        };
        let mut screenshots: Vec<_> = table_screenshots
            .into_iter()
            .flatten()
            .map(FsdImage::classify)
            .collect();
        if screenshots.is_empty() {
            screenshots = entries
                .iter()
                .filter(|entry| entry.asset_type == TYPE_SCREENSHOT)
                .filter_map(|entry| entry.data)
                .map(FsdImage::classify)
                .collect();
        }
        Ok(Self {
            cover: first_of_type(TYPE_COVER),
            background: first_of_type(TYPE_BACKGROUND),
            icon: first_of_type(TYPE_ICON),
            banner: first_of_type(TYPE_BANNER),
            screenshots,
        })
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(std::array::from_fn(|byte| bytes[offset + byte]))
}

/// Asset entry rows are `{type, offset, size, total_size}`.
fn asset_entry(bytes: &[u8], index: usize) -> Result<Entry<'_>, FsdError> {
    let row = ENTRY_TABLE_OFFSET + index * TABLE_ROW_LEN;
    Ok(Entry {
        asset_type: read_u32(bytes, row),
        data: entry_data(
            bytes,
            read_u32(bytes, row + 4),
            read_u32(bytes, row + 8),
            index,
        )?,
    })
}

/// Screenshot entry rows are `{offset, size, total_size, reserved}`.
fn screenshot_entry(bytes: &[u8], index: usize) -> Result<Option<&[u8]>, FsdError> {
    let row = SCREENSHOT_TABLE_OFFSET + index * TABLE_ROW_LEN;
    entry_data(
        bytes,
        read_u32(bytes, row),
        read_u32(bytes, row + 4),
        ENTRY_COUNT + index,
    )
}

/// Every non-empty entry must lie inside the file, used or not, as the old app required.
fn entry_data(
    bytes: &[u8],
    offset: u32,
    size: u32,
    index: usize,
) -> Result<Option<&[u8]>, FsdError> {
    if size == 0 {
        return Ok(None);
    }
    let start = offset as usize;
    start
        .checked_add(size as usize)
        .and_then(|end| bytes.get(start..end))
        .map(Some)
        .ok_or(FsdError::EntryOutOfBounds { index })
}
