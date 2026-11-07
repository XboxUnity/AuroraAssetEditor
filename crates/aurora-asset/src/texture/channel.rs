//! Stored channel values as the floats from 0 to 1 that the decoder and the encoder work with.

use crate::bytes::CHANNEL_MAX;

pub(super) const RED_SHIFT: u32 = 11;
pub(super) const GREEN_SHIFT: u32 = 5;
const RED_BLUE_MASK: u16 = 0b1_1111;
const GREEN_MASK: u16 = 0b11_1111;

/// The highest value of each RGB565 field.
pub(super) const RED_LEVELS: f32 = 31.0;
pub(super) const GREEN_LEVELS: f32 = 63.0;
pub(super) const BLUE_LEVELS: f32 = 31.0;
pub(super) const RED_LEVEL_STEP: f32 = 1.0 / RED_LEVELS;
pub(super) const GREEN_LEVEL_STEP: f32 = 1.0 / GREEN_LEVELS;
pub(super) const BLUE_LEVEL_STEP: f32 = 1.0 / BLUE_LEVELS;

/// The three channels of an RGB565 color, from 0 to 1.
// Derived from AuroraAsset.dll for interoperability: each field times an `f32` reciprocal, in the
// decoder and the encoder alike.
pub(super) fn expand_565(packed: u16) -> [f32; 3] {
    [
        f32::from((packed >> RED_SHIFT) & RED_BLUE_MASK) * RED_LEVEL_STEP,
        f32::from((packed >> GREEN_SHIFT) & GREEN_MASK) * GREEN_LEVEL_STEP,
        f32::from(packed & RED_BLUE_MASK) * BLUE_LEVEL_STEP,
    ]
}

// Derived from AuroraAsset.dll for interoperability: `n * (1 / 255)` in `f32`, not `n / 255`, in
// the decoder and the encoder alike.
pub(super) fn unit_from_byte(value: u8) -> f32 {
    const UNIT_PER_LEVEL: f32 = 1.0 / CHANNEL_MAX;
    f32::from(value) * UNIT_PER_LEVEL
}
