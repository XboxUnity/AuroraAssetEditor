//! What AuroraAsset.dll's colour endpoint optimizer uses when a point projects 1.5 steps or more
//! before the low endpoint. It does not bound the step index, so a negative step reads the stack
//! slots in front of its palette and the constants in front of its two weight tables. Most slots
//! hold the optimizer's own state, which this reproduces; three are never written. See
//! docs/codec/xg-bc3-encoder.md.

// Derived from AuroraAsset.dll for interoperability: the frame and constants before the palette.

use super::{
    BLUE, GREEN, MIN_AXIS_LENGTH_SQUARED, NewtonSums, RED, Rgb, StepRead,
    TWO_COLOR_AXIS_LENGTH_SQUARED,
};

/// The low endpoint's fourth component, copied with it from the luminance weights.
const LOW_ENDPOINT_ALPHA: f32 = 1.0;
/// Frame slots the optimizer never writes. They hold whatever the stack held before, which stays
/// the same within one `ConvertImageToAsset` call but changes from one process to the next, so no
/// fixed value is right. Zero is used.
const NEVER_WRITTEN: f32 = 0.0;

/// The upper halves of the doubles `2^-16` and `1/8`, which sit in front of the weight tables.
const CONVERGENCE_LIMIT_UPPER_HALF: f32 = f32::from_bits(0x3ef0_0000);
const EIGHTH_UPPER_HALF: f32 = f32::from_bits(0x3fc0_0000);
const DOUBLE_LOWER_HALF: f32 = 0.0;

/// The weights read for steps -1, -2, ... The high endpoint's table lies directly in front of
/// the low endpoint's, so the low weights run back through the high table.
const LOW_WEIGHTS_BEFORE_TABLE: [f32; 5] =
    [1.0, 2.0 / 3.0, 1.0 / 3.0, 0.0, MIN_AXIS_LENGTH_SQUARED];
const HIGH_WEIGHTS_BEFORE_TABLE: [f32; 5] = [
    MIN_AXIS_LENGTH_SQUARED,
    TWO_COLOR_AXIS_LENGTH_SQUARED,
    CONVERGENCE_LIMIT_UPPER_HALF,
    DOUBLE_LOWER_HALF,
    EIGHTH_UPPER_HALF,
];

/// The palette entry and weights the DLL reads for a negative `step`, from the optimizer's state
/// at that point of the loop. `None` from step -6 on: step -6 reads the previous point's
/// difference (stack residue for the first point), and step -7 the weight-table pointers, whose
/// values depend on where the DLL is loaded.
pub(super) fn read_before_palette(
    step: i32,
    low: &Rgb,
    high: &Rgb,
    sums: &NewtonSums,
    iteration: usize,
) -> Option<StepRead> {
    let color = match step {
        -1 => [low[BLUE], LOW_ENDPOINT_ALPHA, sums.low_curvature],
        -2 => [NEVER_WRITTEN, sums.high_curvature, low[RED]],
        -3 => [iteration_slot(iteration), high[RED], high[GREEN]],
        -4 => [
            sums.low_gradient[GREEN],
            sums.low_gradient[BLUE],
            NEVER_WRITTEN,
        ],
        -5 => [
            sums.high_gradient[GREEN],
            sums.high_gradient[BLUE],
            NEVER_WRITTEN,
        ],
        _ => return None,
    };
    let table_index = (-step - 1) as usize;
    Some(StepRead {
        color,
        low_weight: LOW_WEIGHTS_BEFORE_TABLE[table_index],
        high_weight: HIGH_WEIGHTS_BEFORE_TABLE[table_index],
    })
}

/// The iteration counter is an integer; the optimizer reads its bits as a float.
fn iteration_slot(iteration: usize) -> f32 {
    f32::from_bits(iteration as u32)
}
