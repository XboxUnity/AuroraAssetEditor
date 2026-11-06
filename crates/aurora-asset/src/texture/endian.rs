/// The byte order the GPU fetch constant declares for a texture's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GpuEndian {
    None,
    EightIn16,
    EightIn32,
    SixteenIn32,
}

impl GpuEndian {
    pub(crate) fn from_field(value: u32) -> Self {
        match value {
            1 => Self::EightIn16,
            2 => Self::EightIn32,
            3 => Self::SixteenIn32,
            _ => Self::None,
        }
    }

    /// Converts big-endian GPU data to native word order in place. Every mode is its own inverse.
    pub(crate) fn to_native(self, data: &mut [u8]) {
        match self {
            Self::None => {}
            Self::EightIn16 => swap_8in16(data),
            Self::EightIn32 => swap_8in32(data),
            Self::SixteenIn32 => swap_16in32(data),
        }
    }
}

/// Swaps the two 16-bit halves of every 32-bit word (GPU endian mode 16in32).
fn swap_16in32(data: &mut [u8]) {
    for word in data.as_chunks_mut::<4>().0.iter_mut() {
        word.rotate_left(2);
    }
}

/// Swaps the two bytes of every 16-bit word (GPU endian mode 8in16).
pub(crate) fn swap_8in16(data: &mut [u8]) {
    for word in data.as_chunks_mut::<2>().0.iter_mut() {
        word.reverse();
    }
}

/// Reverses the four bytes of every 32-bit word (GPU endian mode 8in32).
pub(crate) fn swap_8in32(data: &mut [u8]) {
    for word in data.as_chunks_mut::<4>().0.iter_mut() {
        word.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_8in16_swaps_each_word() {
        let mut data = [1, 2, 3, 4, 5];
        swap_8in16(&mut data);
        assert_eq!(data, [2, 1, 4, 3, 5]);
    }

    #[test]
    fn swap_8in32_reverses_each_word() {
        let mut data = [1, 2, 3, 4, 5, 6];
        swap_8in32(&mut data);
        assert_eq!(data, [4, 3, 2, 1, 5, 6]);
    }
}
