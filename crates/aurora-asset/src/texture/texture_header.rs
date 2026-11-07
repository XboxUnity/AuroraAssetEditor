use super::channel_order::ChannelOrder;
use super::compression::Compression;
use super::gpu_endian::GpuEndian;
use super::surface_layout::{PITCH_UNIT_BYTES, SurfaceLayout};
use super::texture_format::{self, FetchFields, TextureFormat};
use super::{EncodeError, MAX_DIMENSION};
use crate::bytes::read_u32_be;

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

    pub const NUM_FORMAT: BitField = BitField::new(0, 1);
    pub const SIGN: BitField = BitField::new(2, 8);

    /// The low bits of the first dword that select the resource type; flipping any one of them
    /// crashed the DLL.
    pub const COMMON_TYPE: BitField = BitField::new(0, 4);
    /// FETCH_3 bit 31: when set the DLL decodes a texture two pixels wider and taller.
    pub const BORDER_SIZE: BitField = BitField::new(31, 1);
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
        texture_format::classify(&self.fetch_fields())
    }

    pub(crate) fn tiled(&self) -> bool {
        field::TILED.get(self.dword(dword::FETCH_0)) == 1
    }

    pub(crate) fn pitch(&self) -> u32 {
        field::PITCH.get(self.dword(dword::FETCH_0))
    }

    pub(crate) fn data_format(&self) -> u32 {
        field::DATA_FORMAT.get(self.dword(dword::FETCH_1))
    }

    pub(crate) fn endian(&self) -> u32 {
        field::ENDIAN.get(self.dword(dword::FETCH_1))
    }

    pub(crate) fn swizzle_x(&self) -> u32 {
        self.swizzle(0)
    }

    pub(crate) fn swizzle_y(&self) -> u32 {
        self.swizzle(1)
    }

    pub(crate) fn swizzle_z(&self) -> u32 {
        self.swizzle(2)
    }

    pub(crate) fn swizzle_w(&self) -> u32 {
        self.swizzle(3)
    }

    pub(crate) fn num_format(&self) -> u32 {
        field::NUM_FORMAT.get(self.dword(dword::FETCH_3))
    }

    pub(crate) fn dimension(&self) -> u32 {
        field::DIMENSION.get(self.dword(dword::FETCH_5))
    }

    pub(crate) fn sign_bits(&self) -> u32 {
        field::SIGN.get(self.dword(dword::FETCH_0))
    }

    pub(crate) fn gpu_endian(&self) -> GpuEndian {
        GpuEndian::from_field(self.endian())
    }

    pub(crate) fn channel_order(&self) -> Option<ChannelOrder> {
        ChannelOrder::from_swizzle(self.swizzle_channels())
    }

    /// Fields the port does not model but the DLL's decode depends on (found by probing) must hold
    /// the value of the DLL's own headers. Every other unmodelled field was probed and has no
    /// effect.
    fn template_fields_match(&self) -> bool {
        field::COMMON_TYPE.get(self.dword(dword::COMMON)) == COMMON_TEXTURE
            && field::BORDER_SIZE.get(self.dword(dword::FETCH_3)) == 0
            && self.dimension() == GPU_DIMENSION_2D
    }

    fn swizzle_channels(&self) -> [u32; 4] {
        [
            self.swizzle_x(),
            self.swizzle_y(),
            self.swizzle_z(),
            self.swizzle_w(),
        ]
    }

    fn fetch_fields(&self) -> FetchFields {
        FetchFields {
            tiled: self.tiled(),
            data_format: self.data_format(),
            endian: self.endian(),
            swizzle: self.swizzle_channels(),
            num_format: self.num_format(),
            sign_bits: self.sign_bits(),
            template_fields_match: self.template_fields_match(),
        }
    }

    fn swizzle(&self, channel: usize) -> u32 {
        field::SWIZZLE[channel].get(self.dword(dword::FETCH_3))
    }

    fn dword(&self, index: usize) -> u32 {
        read_u32_be(&self.0, index * DWORD_LEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_from_dwords(words: [u32; HEADER_DWORDS]) -> TextureHeader {
        let mut bytes = [0u8; TEXTURE_HEADER_LEN];
        for (chunk, word) in bytes.as_chunks_mut::<DWORD_LEN>().0.iter_mut().zip(words) {
            *chunk = word.to_be_bytes();
        }
        TextureHeader::from_bytes(bytes)
    }

    const DXT_256: [u32; HEADER_DWORDS] = [
        0x3,
        0x1,
        0,
        0,
        0,
        0xFFFF_0000,
        0xFFFF_0000,
        0x0200_0002,
        0x54,
        0x001F_E0FF,
        0x0D10,
        0,
        0x0A00,
    ];
    const ARGB_256: [u32; HEADER_DWORDS] = [
        0x3,
        0x1,
        0,
        0,
        0,
        0xFFFF_0000,
        0xFFFF_0000,
        0x0200_0002,
        0x86,
        0x001F_E0FF,
        0x0C14,
        0,
        0x0A00,
    ];

    #[test]
    fn accessors_read_the_dll_dxt_header() {
        let header = header_from_dwords(DXT_256);
        assert!(!header.tiled());
        assert_eq!(header.pitch(), 8);
        assert_eq!(header.data_format(), 0x14);
        assert_eq!(header.endian(), 1);
        assert_eq!((header.width(), header.height()), (256, 256));
        assert_eq!(
            [
                header.swizzle_x(),
                header.swizzle_y(),
                header.swizzle_z(),
                header.swizzle_w()
            ],
            [0, 1, 2, 3]
        );
        assert_eq!(header.num_format(), 0);
        assert_eq!(header.dimension(), 1);
    }

    #[test]
    fn accessors_read_the_dll_argb_header() {
        let header = header_from_dwords(ARGB_256);
        assert!(!header.tiled());
        assert_eq!(header.pitch(), 8);
        assert_eq!(header.data_format(), 0x06);
        assert_eq!(header.endian(), 2);
        assert_eq!(
            [
                header.swizzle_x(),
                header.swizzle_y(),
                header.swizzle_z(),
                header.swizzle_w()
            ],
            [2, 1, 0, 3]
        );
        assert_eq!(header.num_format(), 0);
        assert_eq!(header.dimension(), 1);
    }

    #[test]
    fn tiled_flag_is_read_from_the_top_bit() {
        let mut words = DXT_256;
        words[dword::FETCH_0] |= 0x8000_0000;
        assert!(header_from_dwords(words).tiled());
    }
}
