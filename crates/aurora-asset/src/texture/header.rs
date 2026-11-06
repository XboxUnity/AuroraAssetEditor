use super::format::{Compression, FetchFormat, TextureFormat};
use super::layout::{PITCH_UNIT_BYTES, SurfaceLayout};
use super::{EncodeError, MAX_DIMENSION};

pub const TEXTURE_HEADER_LEN: usize = 52;

const DWORD_LEN: usize = 4;
const HEADER_DWORDS: usize = TEXTURE_HEADER_LEN / DWORD_LEN;

const COMMON_TEXTURE: u32 = 3;
const REFERENCE_COUNT: u32 = 1;
const FLUSH_ALL: u32 = 0xFFFF_0000;
const GPU_CONSTANT_TYPE_TEXTURE: u32 = 2;
const GPU_DIMENSION_2D: u32 = 1;

mod dword {
    pub const COMMON: usize = 0;
    pub const REFERENCE_COUNT: usize = 1;
    pub const BASE_FLUSH: usize = 5;
    pub const MIP_FLUSH: usize = 6;
    pub const FETCH_0: usize = 7;
    pub const FETCH_1: usize = 8;
    pub const FETCH_2_SIZE: usize = 9;
    pub const FETCH_3: usize = 10;
    pub const FETCH_5: usize = 12;
}

/// A run of bits inside a 32-bit word. Positions are counted from the least
/// significant bit; the `.bt` template lists the same fields most significant first.
#[derive(Clone, Copy)]
struct BitField {
    shift: u32,
    width: u32,
}

impl BitField {
    const fn new(shift: u32, width: u32) -> Self {
        Self { shift, width }
    }

    const fn mask(self) -> u32 {
        (1 << self.width) - 1
    }

    const fn get(self, word: u32) -> u32 {
        (word >> self.shift) & self.mask()
    }

    const fn set(self, word: u32, value: u32) -> u32 {
        (word & !(self.mask() << self.shift)) | ((value & self.mask()) << self.shift)
    }
}

mod field {
    use super::BitField;

    pub const TILED: BitField = BitField::new(31, 1);
    pub const PITCH: BitField = BitField::new(22, 9);
    pub const CONSTANT_TYPE: BitField = BitField::new(0, 2);

    pub const ENDIAN: BitField = BitField::new(6, 2);
    pub const DATA_FORMAT: BitField = BitField::new(0, 6);

    pub const WIDTH_MINUS_ONE: BitField = BitField::new(0, 13);
    pub const HEIGHT_MINUS_ONE: BitField = BitField::new(13, 13);

    pub const SWIZZLE: [BitField; 4] = [
        BitField::new(1, 3),
        BitField::new(4, 3),
        BitField::new(7, 3),
        BitField::new(10, 3),
    ];

    pub const PACKED_MIPS: BitField = BitField::new(11, 1);
    pub const DIMENSION: BitField = BitField::new(9, 2);
}

/// The 52-byte `D3DBaseTexture` header, big-endian, exactly as stored in an `.asset` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureHeader([u8; TEXTURE_HEADER_LEN]);

impl TextureHeader {
    pub fn from_bytes(bytes: [u8; TEXTURE_HEADER_LEN]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(&self) -> [u8; TEXTURE_HEADER_LEN] {
        self.0
    }

    /// The header AuroraAsset.dll writes for a linear texture of this size.
    pub fn for_image(
        width: u32,
        height: u32,
        compression: Compression,
    ) -> Result<Self, EncodeError> {
        let supported = 1..=MAX_DIMENSION;
        if !supported.contains(&width) || !supported.contains(&height) {
            return Err(EncodeError::DimensionsOutOfRange { width, height });
        }

        let layout = SurfaceLayout::for_compression(width, height, compression);
        let format = compression.fetch_format();
        let mut words = [0u32; HEADER_DWORDS];

        words[dword::COMMON] = COMMON_TEXTURE;
        words[dword::REFERENCE_COUNT] = REFERENCE_COUNT;
        words[dword::BASE_FLUSH] = FLUSH_ALL;
        words[dword::MIP_FLUSH] = FLUSH_ALL;

        let pitch_units = layout.row_pitch / PITCH_UNIT_BYTES;
        let fetch_0 = &mut words[dword::FETCH_0];
        *fetch_0 = field::TILED.set(*fetch_0, 0);
        *fetch_0 = field::PITCH.set(*fetch_0, pitch_units as u32);
        *fetch_0 = field::CONSTANT_TYPE.set(*fetch_0, GPU_CONSTANT_TYPE_TEXTURE);

        let fetch_1 = &mut words[dword::FETCH_1];
        *fetch_1 = field::ENDIAN.set(*fetch_1, format.endian);
        *fetch_1 = field::DATA_FORMAT.set(*fetch_1, format.data_format);

        let size = &mut words[dword::FETCH_2_SIZE];
        *size = field::WIDTH_MINUS_ONE.set(*size, width - 1);
        *size = field::HEIGHT_MINUS_ONE.set(*size, height - 1);

        let fetch_3 = &mut words[dword::FETCH_3];
        for (swizzle_field, channel) in field::SWIZZLE.iter().zip(format.swizzle) {
            *fetch_3 = swizzle_field.set(*fetch_3, channel);
        }

        let fetch_5 = &mut words[dword::FETCH_5];
        *fetch_5 = field::PACKED_MIPS.set(*fetch_5, 1);
        *fetch_5 = field::DIMENSION.set(*fetch_5, GPU_DIMENSION_2D);

        let mut bytes = [0u8; TEXTURE_HEADER_LEN];
        for (chunk, word) in bytes.as_chunks_mut::<DWORD_LEN>().0.iter_mut().zip(words) {
            *chunk = word.to_be_bytes();
        }
        Ok(Self(bytes))
    }

    pub fn width(&self) -> u32 {
        field::WIDTH_MINUS_ONE.get(self.dword(dword::FETCH_2_SIZE)) + 1
    }

    pub fn height(&self) -> u32 {
        field::HEIGHT_MINUS_ONE.get(self.dword(dword::FETCH_2_SIZE)) + 1
    }

    pub fn format(&self) -> TextureFormat {
        let tiled = self.tiled();
        let data_format = self.data_format();
        let endian = self.endian();
        let is = |known: FetchFormat| {
            !tiled && data_format == known.data_format && endian == known.endian
        };

        if is(FetchFormat::LINEAR_DXT4) {
            TextureFormat::LinearDxt4
        } else if is(FetchFormat::LINEAR_ARGB8888) {
            TextureFormat::LinearArgb8888
        } else {
            TextureFormat::Unsupported {
                data_format: data_format as u8,
                tiled,
            }
        }
    }

    fn tiled(&self) -> bool {
        field::TILED.get(self.dword(dword::FETCH_0)) == 1
    }

    fn data_format(&self) -> u32 {
        field::DATA_FORMAT.get(self.dword(dword::FETCH_1))
    }

    fn endian(&self) -> u32 {
        field::ENDIAN.get(self.dword(dword::FETCH_1))
    }

    fn dword(&self, index: usize) -> u32 {
        let start = index * DWORD_LEN;
        let mut bytes = [0u8; DWORD_LEN];
        bytes.copy_from_slice(&self.0[start..start + DWORD_LEN]);
        u32::from_be_bytes(bytes)
    }
}
