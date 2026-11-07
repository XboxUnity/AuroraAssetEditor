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
