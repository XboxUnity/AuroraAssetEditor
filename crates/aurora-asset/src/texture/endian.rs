/// Swaps the two bytes of every 16-bit word (GPU endian mode 8in16).
#[cfg_attr(not(test), expect(dead_code, reason = "first used by the DXT encoder"))]
pub(crate) fn swap_8in16(data: &mut [u8]) {
    for word in data.as_chunks_mut::<2>().0.iter_mut() {
        word.reverse();
    }
}

/// Reverses the four bytes of every 32-bit word (GPU endian mode 8in32).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "first used by the ARGB encoder")
)]
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
