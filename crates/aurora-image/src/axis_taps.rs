use image::{Rgba, RgbaImage};

const FIXED_POINT_SHIFT: u32 = 16;
const FIXED_POINT_ONE: u64 = 1 << FIXED_POINT_SHIFT;
/// GDI+ interpolates with 8-bit fractions.
const GDIPLUS_FRACTION_SHIFT: u32 = 8;
const GDIPLUS_FRACTION_ONE: u64 = 1 << GDIPLUS_FRACTION_SHIFT;
const RECIPROCAL_SHIFT: u32 = 32;
const RECIPROCAL_ONE: u64 = 1 << RECIPROCAL_SHIFT;

/// One source pixel and its weight; `None` reads transparent black.
struct Tap {
    index: Option<u32>,
    weight: u64,
}

/// The taps of one output pixel; their weighted sum is divided by `divisor`, rounding half up.
/// The division multiplies by a truncated 0.32 reciprocal, as GDI+'s area averaging does; for
/// the power-of-two divisors of bilinear taps that is exact.
struct OutputTaps {
    taps: Vec<Tap>,
    divisor: u64,
}

impl OutputTaps {
    fn apply(&self, read: impl Fn(u32) -> u8) -> u8 {
        let sum: u64 = self
            .taps
            .iter()
            .filter_map(|tap| tap.index.map(|index| u64::from(read(index)) * tap.weight))
            .sum();
        let reciprocal = RECIPROCAL_ONE / self.divisor;
        ((sum * reciprocal + RECIPROCAL_ONE / 2) >> RECIPROCAL_SHIFT).min(u64::from(u8::MAX)) as u8
    }
}

/// The source pixels and weights each output pixel reads along one axis.
pub(crate) struct AxisTaps {
    per_output: Vec<OutputTaps>,
}

/// The 16.16 source distance between two output pixels.
fn fixed_point_step(source_len: u32, output_len: u32) -> u64 {
    (u64::from(source_len) << FIXED_POINT_SHIFT) / u64::from(output_len)
}

fn rounded_fixed_point_step(source_len: u32, output_len: u32) -> u64 {
    let output = u64::from(output_len);
    ((u64::from(source_len) << FIXED_POINT_SHIFT) + output / 2) / output
}

impl AxisTaps {
    /// Two taps around the sample position, pixel centres aligned (`(x + 0.5) * scale - 0.5`),
    /// edges repeating the nearest pixel, whatever the scale.
    pub(crate) fn centred_bilinear(output_len: u32, source_len: u32) -> Self {
        let scale = f64::from(source_len) / f64::from(output_len);
        let last = f64::from(source_len - 1);
        let per_output = (0..output_len)
            .map(|output| {
                let position = ((f64::from(output) + 0.5) * scale - 0.5).clamp(0.0, last);
                let first = position.floor();
                let next_weight = ((position - first) * FIXED_POINT_ONE as f64).round() as u64;
                let next = (first + 1.0).min(last);
                OutputTaps {
                    taps: vec![
                        Tap {
                            index: Some(first as u32),
                            weight: FIXED_POINT_ONE - next_weight,
                        },
                        Tap {
                            index: Some(next as u32),
                            weight: next_weight,
                        },
                    ],
                    divisor: FIXED_POINT_ONE,
                }
            })
            .collect();
        Self { per_output }
    }

    /// GDI+'s bilinear: pixel origins aligned (`x * step`, the step rounded to 16.16), an 8-bit
    /// fraction, and transparent black past the last pixel.
    pub(crate) fn gdiplus_bilinear(output_len: u32, source_len: u32) -> Self {
        let step = rounded_fixed_point_step(source_len, output_len);
        let per_output = (0..u64::from(output_len))
            .map(|output| {
                let position = output * step;
                let first = position >> FIXED_POINT_SHIFT;
                let fraction = (position >> (FIXED_POINT_SHIFT - GDIPLUS_FRACTION_SHIFT))
                    & (GDIPLUS_FRACTION_ONE - 1);
                let inside = |index: u64| (index < u64::from(source_len)).then_some(index as u32);
                OutputTaps {
                    taps: vec![
                        Tap {
                            index: inside(first),
                            weight: GDIPLUS_FRACTION_ONE - fraction,
                        },
                        Tap {
                            index: inside(first + 1),
                            weight: fraction,
                        },
                    ],
                    divisor: GDIPLUS_FRACTION_ONE,
                }
            })
            .collect();
        Self { per_output }
    }

    /// GDI+'s area averaging: each output pixel is the mean of the source span it covers, the
    /// step truncated to 16.16.
    pub(crate) fn gdiplus_area(output_len: u32, source_len: u32) -> Self {
        let step = fixed_point_step(source_len, output_len);
        let source_end = u64::from(source_len) << FIXED_POINT_SHIFT;
        let per_output = (0..u64::from(output_len))
            .map(|output| {
                let start = output * step;
                let end = (start + step).min(source_end);
                let first = start >> FIXED_POINT_SHIFT;
                let after_last = end.div_ceil(FIXED_POINT_ONE);
                let taps = (first..after_last)
                    .map(|index| {
                        let pixel_start = index << FIXED_POINT_SHIFT;
                        let covered =
                            end.min(pixel_start + FIXED_POINT_ONE) - start.max(pixel_start);
                        Tap {
                            index: Some(index as u32),
                            weight: covered,
                        }
                    })
                    .collect();
                OutputTaps {
                    taps,
                    divisor: end - start,
                }
            })
            .collect();
        Self { per_output }
    }

    fn len(&self) -> u32 {
        self.per_output.len() as u32
    }
}

/// Resamples along x with `columns`, then along y with `rows`, rounding to 8 bits after each
/// pass as GDI+ does.
pub(crate) fn stretch(source: &RgbaImage, columns: &AxisTaps, rows: &AxisTaps) -> RgbaImage {
    let widened = RgbaImage::from_fn(columns.len(), source.height(), |x, y| {
        let taps = &columns.per_output[x as usize];
        Rgba(std::array::from_fn(|channel| {
            taps.apply(|source_x| source.get_pixel(source_x, y)[channel])
        }))
    });
    RgbaImage::from_fn(columns.len(), rows.len(), |x, y| {
        let taps = &rows.per_output[y as usize];
        Rgba(std::array::from_fn(|channel| {
            taps.apply(|source_y| widened.get_pixel(x, source_y)[channel])
        }))
    })
}
