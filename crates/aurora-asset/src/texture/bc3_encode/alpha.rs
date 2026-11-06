//! The alpha half of a BC3 block: two 8-bit endpoints and sixteen 3-bit indices.

use super::x87::Fpu;
use super::{Bc3Options, TEXELS_PER_BLOCK, Texel, diffuse_error, end_weights, spread_error};

const ALPHA_HALF_BYTES: usize = 8;
const INDEX_BYTES_PER_SET: usize = 3;
const TEXELS_PER_INDEX_SET: usize = 8;
const INDEX_BITS: u32 = 3;
/// A set's eight indices fill 24 bits; each new one enters at the top and the rest shift down.
const INDEX_ENTRY_SHIFT: u32 = 24 - INDEX_BITS;

const ALPHA_LEVELS: f32 = 255.0;
const ALPHA_LEVEL_STEP: f32 = 1.0 / 255.0;
const ALPHA_MIN: f32 = 0.0;
const ALPHA_MAX: f32 = 1.0;
const ROUNDING: f32 = 0.5;

const SIX_STEP_COUNT: usize = 6;
const EIGHT_STEP_COUNT: usize = 8;
const SIX_STEP_RESERVED_LOW: usize = 6;
const SIX_STEP_RESERVED_HIGH: usize = 7;

/// Maps a position along the endpoint axis to the index code that selects it.
const POSITION_TO_CODE_6: [usize; 6] = [0, 2, 3, 4, 5, 1];
const POSITION_TO_CODE_8: [usize; 8] = [0, 2, 3, 4, 5, 6, 7, 1];

const REFINE_ITERATIONS: usize = 8;
const MIN_REFINE_SPAN: f32 = 1.0 / 256.0;
const REFINE_CONVERGED_SQUARED_STEP: f32 = 1.0 / 64.0;

// Derived from AuroraAsset.dll for interoperability: these loops run with the FPU truncating.
const QUANTIZE_FPU: Fpu = Fpu::RoundTowardZero;
const REFINE_FPU: Fpu = Fpu::RoundTowardZero;
const ENDPOINT_FPU: Fpu = Fpu::RoundTowardZero;
const INDEX_FPU: Fpu = Fpu::RoundTowardZero;
// Derived from AuroraAsset.dll for interoperability: step interpolation and index scale round to nearest.
const STEP_FPU: Fpu = Fpu::RoundToNearest;

struct QuantizedAlpha {
    levels: [f32; TEXELS_PER_BLOCK],
    min: f32,
    max: f32,
}

pub(super) fn encode_alpha_block(
    texels: &[Texel; TEXELS_PER_BLOCK],
    options: &Bc3Options,
) -> [u8; ALPHA_HALF_BYTES] {
    let alphas: [f32; TEXELS_PER_BLOCK] = std::array::from_fn(|index| texels[index][3]);
    let quantized = quantize_alpha(&alphas, options.dither);

    let mut block = [0u8; ALPHA_HALF_BYTES];
    if quantized.min == ALPHA_MAX {
        block[0] = u8::MAX;
        block[1] = u8::MAX;
        return block;
    }

    let has_reserved_extremes = quantized.min == ALPHA_MIN || quantized.max == ALPHA_MAX;
    let step_count = if has_reserved_extremes {
        SIX_STEP_COUNT
    } else {
        EIGHT_STEP_COUNT
    };

    let (low, high) = refine_alpha_endpoints(&quantized.levels, step_count);
    let low_byte = alpha_to_level(low, ENDPOINT_FPU) as u8;
    let high_byte = alpha_to_level(high, ENDPOINT_FPU) as u8;
    let low = f32::from(low_byte) * ALPHA_LEVEL_STEP;
    let high = f32::from(high_byte) * ALPHA_LEVEL_STEP;

    if step_count == EIGHT_STEP_COUNT && low_byte == high_byte {
        block[0] = low_byte;
        block[1] = high_byte;
        return block;
    }

    // Six-step blocks store the low endpoint first; eight-step blocks store the high one first.
    let (first_byte, second_byte, first, second) = if step_count == SIX_STEP_COUNT {
        (low_byte, high_byte, low, high)
    } else {
        (high_byte, low_byte, high, low)
    };
    block[0] = first_byte;
    block[1] = second_byte;

    let steps = alpha_steps(first, second, step_count);
    assign_alpha_indices(&mut block, texels, &steps, step_count, options.dither);
    block
}

/// Rounds each alpha to 8 bits (optionally diffusing the rounding error) and finds the range.
fn quantize_alpha(alphas: &[f32; TEXELS_PER_BLOCK], dither: bool) -> QuantizedAlpha {
    let fpu = QUANTIZE_FPU;
    let mut levels = [0.0f32; TEXELS_PER_BLOCK];
    let mut errors = [0.0f32; TEXELS_PER_BLOCK];
    let mut min = alphas[0];
    let mut max = alphas[0];

    for index in 0..TEXELS_PER_BLOCK {
        let mut alpha = alphas[index];
        if dither {
            alpha = (fpu.load(errors[index]) + alpha).store();
        }
        let level = alpha_to_level(alpha, fpu) as f32;
        levels[index] = (fpu.load(level) * ALPHA_LEVEL_STEP).store();

        if levels[index] < min {
            min = levels[index];
        } else if levels[index] > max {
            max = levels[index];
        }

        if dither {
            let difference = (fpu.load(alpha) - levels[index]).store();
            spread_error(index, |target, weight| {
                errors[target] = diffuse_error(errors[target], difference, weight)
            });
        }
    }
    QuantizedAlpha { levels, min, max }
}

/// The nearest 8-bit level; a dithered alpha can land just outside 0..=255.
fn alpha_to_level(alpha: f32, fpu: Fpu) -> i32 {
    (fpu.load(alpha) * ALPHA_LEVELS + ROUNDING).store() as i32
}

/// Newton iterations on the sum-of-squares error, from the range of the usable alpha values.
fn refine_alpha_endpoints(points: &[f32; TEXELS_PER_BLOCK], step_count: usize) -> (f32, f32) {
    let fpu = REFINE_FPU;
    let (end_weights, other_end_weights) = end_weights::<EIGHT_STEP_COUNT>(step_count);
    let (mut low, mut high) = usable_range(points, step_count);

    let last_position = (step_count - 1) as f32;
    for _ in 0..REFINE_ITERATIONS {
        let span = fpu.load(high) - low;
        if span < MIN_REFINE_SPAN {
            break;
        }
        let scale = (fpu.load(last_position) / span).store();

        let mut steps = [0.0f32; EIGHT_STEP_COUNT];
        for step in 0..step_count {
            // Derived from AuroraAsset.dll for interoperability: the high endpoint's share comes first.
            let from_high = fpu.load(other_end_weights[step]) * high;
            steps[step] = (from_high + fpu.load(end_weights[step]) * low).store();
        }
        if step_count == SIX_STEP_COUNT {
            steps[SIX_STEP_RESERVED_LOW] = ALPHA_MIN;
            steps[SIX_STEP_RESERVED_HIGH] = ALPHA_MAX;
        }

        let mut low_gradient = 0.0f32;
        let mut high_gradient = 0.0f32;
        let mut low_curvature = 0.0f32;
        let mut high_curvature = 0.0f32;

        for &point in points {
            let position = ((fpu.load(point) - low) * scale).store();
            let step = nearest_step(point, position, low, high, step_count);
            if step < step_count {
                let low_weight = end_weights[step];
                let high_weight = other_end_weights[step];
                let difference = (fpu.load(steps[step]) - point).store();
                low_gradient = (fpu.load(difference) * low_weight + low_gradient).store();
                low_curvature = (fpu.load(low_weight) * low_weight + low_curvature).store();
                high_gradient = (fpu.load(high_weight) * difference + high_gradient).store();
                high_curvature = (fpu.load(high_weight) * high_weight + high_curvature).store();
            }
        }

        if low_curvature > 0.0 {
            low = (fpu.load(low) - fpu.load(low_gradient) / low_curvature).store();
        }
        if high_curvature > 0.0 {
            high = (fpu.load(high) - fpu.load(high_gradient) / high_curvature).store();
        }
        if low > high {
            std::mem::swap(&mut low, &mut high);
        }

        if fpu.load(low_gradient) * low_gradient < REFINE_CONVERGED_SQUARED_STEP
            && fpu.load(high_gradient) * high_gradient < REFINE_CONVERGED_SQUARED_STEP
        {
            break;
        }
    }

    (
        low.clamp(ALPHA_MIN, ALPHA_MAX),
        high.clamp(ALPHA_MIN, ALPHA_MAX),
    )
}

/// The starting endpoints: the range of the points, leaving out 0 and 1 in six-step blocks.
fn usable_range(points: &[f32; TEXELS_PER_BLOCK], step_count: usize) -> (f32, f32) {
    let mut low = ALPHA_MAX;
    let mut high = ALPHA_MIN;
    if step_count == EIGHT_STEP_COUNT {
        for &point in points {
            if point < low {
                low = point;
            }
            if point > high {
                high = point;
            }
        }
    } else {
        for &point in points {
            if point < low && point > ALPHA_MIN {
                low = point;
            }
            if point > high && point < ALPHA_MAX {
                high = point;
            }
        }
        if low == high {
            high = ALPHA_MAX;
        }
    }
    (low, high)
}

/// The step a point is closest to; in six-step blocks, the reserved 0 and 1 steps may win.
fn nearest_step(point: f32, position: f32, low: f32, high: f32, step_count: usize) -> usize {
    let fpu = REFINE_FPU;
    let last_position = (step_count - 1) as f32;
    let is_six_step = step_count == SIX_STEP_COUNT;
    if position <= 0.0 {
        let reserved_below = (fpu.load(low) * ROUNDING).store();
        if is_six_step && point <= reserved_below {
            SIX_STEP_RESERVED_LOW
        } else {
            0
        }
    } else if position >= last_position {
        let reserved_above = ((fpu.load(high) + ALPHA_MAX) * ROUNDING).store();
        if is_six_step && point >= reserved_above {
            SIX_STEP_RESERVED_HIGH
        } else {
            step_count - 1
        }
    } else {
        (fpu.load(position) + ROUNDING).store() as u32 as usize
    }
}

/// The alpha value each index code stands for, given the two stored endpoint values.
fn alpha_steps(first: f32, second: f32, step_count: usize) -> [f32; EIGHT_STEP_COUNT] {
    let mut steps = [0.0f32; EIGHT_STEP_COUNT];
    steps[0] = first;
    steps[1] = second;
    let interpolated = step_count - 1;
    let fpu = STEP_FPU;
    // Derived from AuroraAsset.dll for interoperability: multiplies by an `f32` reciprocal instead of dividing.
    let reciprocal = 1.0 / interpolated as f32;
    for index in 1..interpolated {
        let first_share = fpu.load(steps[0]) * (interpolated - index) as f32;
        let second_share = fpu.load(steps[1]) * index as f32;
        steps[index + 1] = ((first_share + second_share) * reciprocal).store();
    }
    if step_count == SIX_STEP_COUNT {
        steps[SIX_STEP_RESERVED_LOW] = ALPHA_MIN;
        steps[SIX_STEP_RESERVED_HIGH] = ALPHA_MAX;
    }
    steps
}

/// Picks each texel's index from its original (unquantized) alpha and packs the two 24-bit sets.
fn assign_alpha_indices(
    block: &mut [u8; ALPHA_HALF_BYTES],
    texels: &[Texel; TEXELS_PER_BLOCK],
    steps: &[f32; EIGHT_STEP_COUNT],
    step_count: usize,
    dither: bool,
) {
    let fpu = INDEX_FPU;
    let position_to_code: &[usize] = if step_count == SIX_STEP_COUNT {
        &POSITION_TO_CODE_6
    } else {
        &POSITION_TO_CODE_8
    };
    let last_position = (step_count - 1) as f32;
    let is_six_step = step_count == SIX_STEP_COUNT;
    let scale = if steps[0] != steps[1] {
        let scale_fpu = STEP_FPU;
        (scale_fpu.load(last_position) / (scale_fpu.load(steps[1]) - steps[0])).store()
    } else {
        0.0
    };

    let mut errors = [0.0f32; TEXELS_PER_BLOCK];
    for set in 0..TEXELS_PER_BLOCK / TEXELS_PER_INDEX_SET {
        let mut packed = 0u32;
        let first_texel = set * TEXELS_PER_INDEX_SET;
        for index in first_texel..first_texel + TEXELS_PER_INDEX_SET {
            let mut alpha = texels[index][3];
            if dither {
                alpha = (fpu.load(errors[index]) + alpha).store();
            }
            let position = ((fpu.load(alpha) - steps[0]) * scale).store();

            let code = if position <= 0.0 {
                if is_six_step && fpu.load(steps[0]) * ROUNDING >= alpha {
                    SIX_STEP_RESERVED_LOW
                } else {
                    0
                }
            } else if position >= last_position {
                if is_six_step && (fpu.load(steps[1]) + ALPHA_MAX) * ROUNDING <= alpha {
                    SIX_STEP_RESERVED_HIGH
                } else {
                    1
                }
            } else {
                position_to_code[(fpu.load(position) + ROUNDING).store() as u32 as usize]
            };

            packed = ((code as u32) << INDEX_ENTRY_SHIFT) | (packed >> INDEX_BITS);

            if dither {
                let difference = (fpu.load(alpha) - steps[code]).store();
                spread_error(index, |target, weight| {
                    errors[target] = diffuse_error(errors[target], difference, weight)
                });
            }
        }

        let start = 2 + set * INDEX_BYTES_PER_SET;
        block[start..start + INDEX_BYTES_PER_SET]
            .copy_from_slice(&packed.to_le_bytes()[..INDEX_BYTES_PER_SET]);
    }
}
