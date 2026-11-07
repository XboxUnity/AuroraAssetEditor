use aurora_asset::{DecodeError, EncodedTexture, decode};

use super::rgba_sha256;

/// What a surface row is made of: 4x4 blocks of 8 or 16 bytes, or 4-byte texels.
#[derive(Clone, Copy)]
pub struct SurfaceUnit {
    pub texels_per_side: usize,
    pub bytes: usize,
}

pub const BC1_BLOCK: SurfaceUnit = SurfaceUnit {
    texels_per_side: 4,
    bytes: 8,
};
pub const BC3_BLOCK: SurfaceUnit = SurfaceUnit {
    texels_per_side: 4,
    bytes: 16,
};
pub const ARGB_TEXEL: SurfaceUnit = SurfaceUnit {
    texels_per_side: 1,
    bytes: 4,
};

const FETCH_0_OFFSET: usize = 28;
const PITCH_SHIFT: u32 = 22;
const PITCH_MASK: u32 = 0x1FF;
const TEXELS_PER_PITCH_UNIT: usize = 32;

/// The bytes a linear decode reads (ruling R32): every row above the last at the header's row
/// pitch, then only the visible units of the last row.
pub fn minimum_data_len(texture: &EncodedTexture, unit: SurfaceUnit) -> usize {
    let header = texture.header.to_bytes();
    let fetch_0 = u32::from_be_bytes(
        header[FETCH_0_OFFSET..FETCH_0_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    let pitch_units = ((fetch_0 >> PITCH_SHIFT) & PITCH_MASK) as usize;
    let row_pitch = pitch_units * (TEXELS_PER_PITCH_UNIT / unit.texels_per_side) * unit.bytes;
    let units_wide = (texture.header.width() as usize).div_ceil(unit.texels_per_side);
    let units_high = (texture.header.height() as usize).div_ceil(unit.texels_per_side);
    (units_high - 1) * row_pitch + units_wide * unit.bytes
}

/// The data cut to the minimum length must decode to `decoded_sha256`, and one byte less must be
/// refused as truncated.
pub fn assert_decodes_at_minimum_len(
    texture: &EncodedTexture,
    unit: SurfaceUnit,
    decoded_sha256: &str,
    label: &str,
) {
    let minimum = minimum_data_len(texture, unit);
    let cut_to = |len: usize| EncodedTexture {
        header: texture.header.clone(),
        data: texture.data[..len].to_vec(),
    };
    let decoded = decode(&cut_to(minimum)).unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        rgba_sha256(decoded.as_raw()),
        decoded_sha256,
        "{label} at {minimum} bytes"
    );
    assert_eq!(
        decode(&cut_to(minimum - 1)).err(),
        Some(DecodeError::TruncatedData {
            expected: minimum,
            actual: minimum - 1
        }),
        "{label}"
    );
}
