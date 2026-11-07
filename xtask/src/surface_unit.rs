/// What a surface row is made of: 4x4 blocks of 8 or 16 bytes, or 4-byte texels.
#[derive(Clone, Copy)]
pub struct SurfaceUnit {
    pub texels_per_side: usize,
    pub bytes: usize,
}
