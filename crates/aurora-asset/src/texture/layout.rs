use super::DecodeError;
use super::format::{Compression, TextureFormat};
use super::header::TextureHeader;

/// One GPU pitch unit is 32 texels: 128 bytes of ARGB, or 8 compressed blocks (128 bytes of
/// BC3, 64 bytes of DXT1).
pub(super) const PITCH_UNIT_BYTES: usize = 128;
const BLOCKS_PER_PITCH_UNIT: usize = 8;

const BLOCKS_PER_ROW_ALIGNMENT: usize = 32;
const ROW_PITCH_ALIGNMENT_BYTES: usize = 256;
const BLOCK_ROWS_ALIGNMENT: usize = 32;

#[derive(Clone, Copy)]
struct BlockShape {
    texels_per_side: usize,
    bytes: usize,
}

const DXT_BLOCK: BlockShape = BlockShape {
    texels_per_side: 4,
    bytes: 16,
};
const DXT1_BLOCK: BlockShape = BlockShape {
    texels_per_side: 4,
    bytes: 8,
};
const ARGB_TEXEL: BlockShape = BlockShape {
    texels_per_side: 1,
    bytes: 4,
};

/// Size of the zero-padded surface the DLL writes for a texture.
pub(crate) struct SurfaceLayout {
    pub row_pitch: usize,
    pub rows: usize,
}

impl SurfaceLayout {
    pub(crate) fn for_header(header: &TextureHeader) -> Result<Self, DecodeError> {
        let format = header.format();
        match format {
            TextureFormat::LinearDxt1 => Self::for_block_header(header, DXT1_BLOCK),
            TextureFormat::LinearDxt4 | TextureFormat::LinearDxt5 => {
                Self::for_block_header(header, DXT_BLOCK)
            }
            TextureFormat::LinearArgb8888 => {
                Ok(Self::new(header.width(), header.height(), ARGB_TEXEL))
            }
            TextureFormat::Unsupported { .. } => Err(DecodeError::UnsupportedFormat(format)),
        }
    }

    /// Block formats take their row pitch from the header, as the DLL does.
    fn for_block_header(header: &TextureHeader, shape: BlockShape) -> Result<Self, DecodeError> {
        let blocks_wide = (header.width() as usize).div_ceil(shape.texels_per_side);
        let required_units = blocks_wide.div_ceil(BLOCKS_PER_PITCH_UNIT);
        let pitch_units = header.pitch() as usize;
        if pitch_units < required_units {
            return Err(DecodeError::RowPitchTooSmall {
                pitch_units: header.pitch(),
                required_units: required_units as u32,
            });
        }
        let blocks_high = (header.height() as usize).div_ceil(shape.texels_per_side);
        Ok(Self {
            row_pitch: pitch_units * BLOCKS_PER_PITCH_UNIT * shape.bytes,
            rows: align(blocks_high, BLOCK_ROWS_ALIGNMENT),
        })
    }

    pub(super) fn for_compression(width: u32, height: u32, compression: Compression) -> Self {
        let shape = match compression {
            Compression::Dxt => DXT_BLOCK,
            Compression::None => ARGB_TEXEL,
        };
        Self::new(width, height, shape)
    }

    fn new(width: u32, height: u32, shape: BlockShape) -> Self {
        let blocks_wide = (width as usize).div_ceil(shape.texels_per_side);
        let blocks_high = (height as usize).div_ceil(shape.texels_per_side);
        let row_bytes = align(blocks_wide, BLOCKS_PER_ROW_ALIGNMENT) * shape.bytes;
        Self {
            row_pitch: align(row_bytes, ROW_PITCH_ALIGNMENT_BYTES),
            rows: align(blocks_high, BLOCK_ROWS_ALIGNMENT),
        }
    }

    pub(crate) fn padded_len(&self) -> usize {
        self.row_pitch * self.rows
    }
}

fn align(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_design_table() {
        let table = [
            (64, 64, Compression::Dxt, 16_384),
            (64, 64, Compression::None, 16_384),
            (420, 96, Compression::Dxt, 65_536),
            (420, 96, Compression::None, 172_032),
            (900, 600, Compression::Dxt, 655_360),
            (900, 600, Compression::None, 2_334_720),
            (1000, 562, Compression::Dxt, 655_360),
            (1000, 562, Compression::None, 2_359_296),
            (1280, 720, Compression::Dxt, 983_040),
            (1280, 720, Compression::None, 3_768_320),
            (256, 256, Compression::Dxt, 65_536),
            (256, 256, Compression::None, 262_144),
        ];
        for (width, height, compression, padded_len) in table {
            let header = TextureHeader::for_image(width, height, compression).unwrap();
            assert_eq!(
                SurfaceLayout::for_header(&header).unwrap().padded_len(),
                padded_len,
                "{width}x{height} {compression:?}"
            );
        }
    }

    #[test]
    fn block_pitch_comes_from_the_header_and_must_hold_a_row() {
        const FETCH_0_OFFSET: usize = 28;
        const PITCH_SHIFT: u32 = 22;
        const PITCH_MASK: u32 = 0x1FF;
        let with_pitch = |units: u32| {
            let mut bytes = TextureHeader::for_image(130, 70, Compression::Dxt)
                .unwrap()
                .to_bytes();
            let word_bytes = &mut bytes[FETCH_0_OFFSET..FETCH_0_OFFSET + 4];
            let word = u32::from_be_bytes((&*word_bytes).try_into().unwrap());
            let word = (word & !(PITCH_MASK << PITCH_SHIFT)) | (units << PITCH_SHIFT);
            word_bytes.copy_from_slice(&word.to_be_bytes());
            TextureHeader::from_bytes(bytes)
        };
        assert_eq!(
            SurfaceLayout::for_header(&with_pitch(12))
                .unwrap()
                .row_pitch,
            12 * PITCH_UNIT_BYTES
        );
        assert_eq!(
            SurfaceLayout::for_header(&with_pitch(4)).err(),
            Some(DecodeError::RowPitchTooSmall {
                pitch_units: 4,
                required_units: 5
            })
        );
    }

    #[test]
    fn unsupported_header_has_no_layout() {
        let mut bytes = TextureHeader::for_image(64, 64, Compression::Dxt)
            .unwrap()
            .to_bytes();
        bytes[32..36].copy_from_slice(&0x0000_003Fu32.to_be_bytes());
        let header = TextureHeader::from_bytes(bytes);
        assert_eq!(
            SurfaceLayout::for_header(&header).err(),
            Some(DecodeError::UnsupportedFormat(header.format()))
        );
    }
}
