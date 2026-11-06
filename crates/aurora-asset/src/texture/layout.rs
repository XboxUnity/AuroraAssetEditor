use super::format::{Compression, TextureFormat};
use super::header::TextureHeader;

/// One GPU pitch unit is 32 texels: 128 bytes of ARGB, or 8 DXT blocks.
pub(super) const PITCH_UNIT_BYTES: usize = 128;

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
    #[cfg_attr(not(test), expect(dead_code, reason = "first used by the decoder"))]
    pub(crate) fn for_header(header: &TextureHeader) -> Option<Self> {
        let shape = match header.format() {
            TextureFormat::LinearDxt1 => DXT1_BLOCK,
            TextureFormat::LinearDxt4 | TextureFormat::LinearDxt5 => DXT_BLOCK,
            TextureFormat::LinearArgb8888 => ARGB_TEXEL,
            TextureFormat::Unsupported { .. } => return None,
        };
        Some(Self::new(header.width(), header.height(), shape))
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

    #[cfg_attr(not(test), expect(dead_code, reason = "first used by the decoder"))]
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
    fn unsupported_header_has_no_layout() {
        let mut bytes = TextureHeader::for_image(64, 64, Compression::Dxt)
            .unwrap()
            .to_bytes();
        bytes[32..36].copy_from_slice(&0x0000_003Fu32.to_be_bytes());
        let header = TextureHeader::from_bytes(bytes);
        assert!(SurfaceLayout::for_header(&header).is_none());
    }
}
