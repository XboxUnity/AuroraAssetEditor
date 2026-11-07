const MAGIC: u32 = 0x4653_4441;
pub const HEADER_LEN: usize = 0x1F8;
pub const ENTRY_TABLE_OFFSET: usize = 24;
const SCREENSHOT_TABLE_OFFSET: usize = 184;
const TABLE_ROW_LEN: usize = 16;

pub const ICON: u32 = 0x01;
pub const BACKGROUND: u32 = 0x02;
pub const BANNER: u32 = 0x04;
pub const BOXART: u32 = 0x08;
pub const PREVIEW: u32 = 0x10;
pub const SCREENSHOT: u32 = 0x20;
pub const SLOT: u32 = 0x40;
pub const COVER: u32 = 0x80;

/// Builds an FSD file: a header with the given entries, then each entry's data in order.
pub struct FsdBuilder {
    bytes: Vec<u8>,
    entries: usize,
    screenshots: usize,
}

impl FsdBuilder {
    pub fn new() -> Self {
        let mut bytes = vec![0; HEADER_LEN];
        bytes[0..4].copy_from_slice(&MAGIC.to_be_bytes());
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        Self {
            bytes,
            entries: 0,
            screenshots: 0,
        }
    }

    fn put_u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn append(&mut self, data: &[u8]) -> (u32, u32) {
        let offset = self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        (offset, data.len() as u32)
    }

    pub fn entry(mut self, asset_type: u32, data: &[u8]) -> Self {
        let (offset, size) = self.append(data);
        let row = ENTRY_TABLE_OFFSET + self.entries * TABLE_ROW_LEN;
        self.put_u32(row, asset_type);
        self.put_u32(row + 4, offset);
        self.put_u32(row + 8, size);
        self.put_u32(row + 12, size);
        self.entries += 1;
        self
    }

    pub fn screenshot(mut self, data: &[u8]) -> Self {
        let (offset, size) = self.append(data);
        let row = SCREENSHOT_TABLE_OFFSET + self.screenshots * TABLE_ROW_LEN;
        self.put_u32(row, offset);
        self.put_u32(row + 4, size);
        self.put_u32(row + 8, size);
        self.screenshots += 1;
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.bytes
    }
}

impl Default for FsdBuilder {
    fn default() -> Self {
        Self::new()
    }
}
