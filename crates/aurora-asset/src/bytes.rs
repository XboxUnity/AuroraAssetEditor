//! Byte readers and pixel sizes shared by the texture, DDS and container code.

pub(crate) const BYTES_PER_PIXEL: usize = 4;
/// Texels per side of a BC1 or BC3 block.
pub(crate) const BLOCK_SIDE: usize = 4;
/// The largest 8-bit channel value, as the factor between a byte and a value from 0 to 1.
pub(crate) const CHANNEL_MAX: f32 = 255.0;

const U32_BYTES: usize = 4;

/// The first `LEN` bytes of `bytes`; the caller has checked that there are that many.
pub(crate) fn block_array<const LEN: usize>(bytes: &[u8]) -> [u8; LEN] {
    let mut array = [0u8; LEN];
    array.copy_from_slice(&bytes[..LEN]);
    array
}

pub(crate) fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(block_array::<U32_BYTES>(&bytes[offset..]))
}

pub(crate) fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(block_array::<U32_BYTES>(&bytes[offset..]))
}
