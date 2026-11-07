use super::BYTES_PER_PIXEL;
use crate::minimum_read::SurfaceUnit;

const BC3_BLOCK_SIDE: usize = 4;
const BC3_BLOCK_BYTES: usize = 16;

#[derive(Clone, Copy)]
pub(super) enum Compression {
    Dxt,
    None,
}

impl Compression {
    pub(super) fn harness_name(self) -> &'static str {
        match self {
            Compression::Dxt => "dxt",
            Compression::None => "none",
        }
    }

    pub(super) fn surface_unit(self) -> SurfaceUnit {
        match self {
            Compression::Dxt => SurfaceUnit {
                texels_per_side: BC3_BLOCK_SIDE,
                bytes: BC3_BLOCK_BYTES,
            },
            Compression::None => SurfaceUnit {
                texels_per_side: 1,
                bytes: BYTES_PER_PIXEL,
            },
        }
    }
}
