use super::fetch_format::GPU_SWIZZLE_A;

/// Which decoded channel each presented channel (x, y, z, w) shows, from the fetch swizzle.
/// The DLL honours any selection of the four stored channels; constant 0 and 1 are not supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChannelOrder([usize; 4]);

impl ChannelOrder {
    pub(super) fn from_swizzle(swizzle: [u32; 4]) -> Option<Self> {
        let selects_stored_channel = |selector: u32| selector <= GPU_SWIZZLE_A;
        swizzle
            .iter()
            .all(|selector| selects_stored_channel(*selector))
            .then(|| Self(swizzle.map(|selector| selector as usize)))
    }

    /// Reorders decoded RGBA values to the presented channel order.
    pub(super) fn arrange(self, values: [f32; 4]) -> [f32; 4] {
        self.0.map(|selector| values[selector])
    }
}
