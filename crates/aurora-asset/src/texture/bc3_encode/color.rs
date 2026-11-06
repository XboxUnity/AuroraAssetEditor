//! The color half of a BC3 block: two RGB565 endpoints and sixteen 2-bit indices (BC1 four-color
//! mode). Endpoints are searched in a perceptually weighted color space.

use super::{Bc3Options, TEXELS_PER_BLOCK, Texel, end_weights, spread_error};

const COLOR_HALF_BYTES: usize = 8;
const INDEX_BITS: u32 = 2;
const INDEX_ENTRY_SHIFT: u32 = 32 - INDEX_BITS;

type Rgb = [f32; 3];
const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;

const RED_LEVELS: f32 = 31.0;
const GREEN_LEVELS: f32 = 63.0;
const BLUE_LEVELS: f32 = 31.0;
const RED_LEVEL_STEP: f32 = 1.0 / 31.0;
const GREEN_LEVEL_STEP: f32 = 1.0 / 63.0;
const BLUE_LEVEL_STEP: f32 = 1.0 / 31.0;
const RED_SHIFT: u32 = 11;
const GREEN_SHIFT: u32 = 5;
const ROUNDING: f32 = 0.5;

/// Perceptual importance of each channel, relative to green.
const LUMINANCE: Rgb = [0.2125 / 0.7154, 1.0, 0.0721 / 0.7154];
const LUMINANCE_INVERSE: Rgb = [0.7154 / 0.2125, 1.0, 0.7154 / 0.0721];
const UNIFORM_WEIGHTS: Rgb = [1.0, 1.0, 1.0];

const STEP_COUNT: usize = 4;
const LAST_POSITION: f32 = (STEP_COUNT - 1) as f32;
/// Maps a position along the endpoint axis to the 2-bit index code that selects it.
const POSITION_TO_CODE: [usize; STEP_COUNT] = [0, 2, 3, 1];
const ONE_THIRD: f32 = 1.0 / 3.0;
const TWO_THIRDS: f32 = 2.0 / 3.0;

const MIN_AXIS_LENGTH_SQUARED: f32 = f32::MIN_POSITIVE;
const TWO_COLOR_AXIS_LENGTH_SQUARED: f32 = 1.0 / 4096.0;
const REFINE_ITERATIONS: usize = 8;
const MIN_REFINE_LENGTH_SQUARED: f32 = 1.0 / 4096.0;
const WEIGHT_SCALE: f32 = 1.0 / 8.0;
const REFINE_CONVERGED_SQUARED_STEP: f32 = (0.25 / 64.0) * (0.25 / 64.0);

/// The two ends of the color axis, in the weighted color space.
struct Endpoints {
    first: Rgb,
    second: Rgb,
}

/// A block's 565 endpoints and the palette they decode to, in the weighted color space.
struct Palette {
    stored_first: u16,
    stored_second: u16,
    colors: [Rgb; STEP_COUNT],
}

pub(super) fn encode_color_block(
    texels: &[Texel; TEXELS_PER_BLOCK],
    options: &Bc3Options,
) -> [u8; COLOR_HALF_BYTES] {
    let weights = if options.uniform_weighting {
        UNIFORM_WEIGHTS
    } else {
        LUMINANCE
    };
    let inverse_weights = if options.uniform_weighting {
        UNIFORM_WEIGHTS
    } else {
        LUMINANCE_INVERSE
    };

    let points = prepare_block(texels, weights, options.dither_rgb);
    let chosen = choose_endpoints(&points, weights);
    let endpoints = refine_endpoints(&points, chosen);

    let first = encode_565(scale_rgb(endpoints.first, inverse_weights));
    let second = encode_565(scale_rgb(endpoints.second, inverse_weights));
    if first == second {
        return pack_color_block(first, second, 0);
    }

    let palette = build_palette(first, second, weights);
    let indices = assign_indices(texels, &palette, weights, options.dither_rgb);
    pack_color_block(palette.stored_first, palette.stored_second, indices)
}

/// Rounds the block's colors to RGB565 levels, diffusing the rounding error when dithering, and
/// moves them into the weighted space the endpoint search runs in.
fn prepare_block(
    texels: &[Texel; TEXELS_PER_BLOCK],
    weights: Rgb,
    dither: bool,
) -> [Rgb; TEXELS_PER_BLOCK] {
    let mut points = [[0.0f32; 3]; TEXELS_PER_BLOCK];
    let mut errors = [[0.0f32; 3]; TEXELS_PER_BLOCK];

    for (index, texel) in texels.iter().enumerate() {
        let mut wanted = [texel[RED], texel[GREEN], texel[BLUE]];
        if dither {
            for channel in 0..3 {
                wanted[channel] += errors[index][channel];
            }
        }

        let rounded = [
            (wanted[RED] * RED_LEVELS + ROUNDING) as i32 as f32 * RED_LEVEL_STEP,
            (wanted[GREEN] * GREEN_LEVELS + ROUNDING) as i32 as f32 * GREEN_LEVEL_STEP,
            (wanted[BLUE] * BLUE_LEVELS + ROUNDING) as i32 as f32 * BLUE_LEVEL_STEP,
        ];

        if dither {
            let difference = subtract(wanted, rounded);
            spread_error(index, |target, weight| {
                for channel in 0..3 {
                    errors[target][channel] += difference[channel] * weight;
                }
            });
        }

        points[index] = scale_rgb(rounded, weights);
    }
    points
}

/// Starting endpoints: the corners of the points' bounding box, choosing the box diagonal that
/// the points follow most closely.
fn choose_endpoints(points: &[Rgb; TEXELS_PER_BLOCK], weights: Rgb) -> Endpoints {
    let mut low = weights;
    let mut high = [0.0f32; 3];
    for point in points {
        for channel in 0..3 {
            if point[channel] < low[channel] {
                low[channel] = point[channel];
            }
            if point[channel] > high[channel] {
                high[channel] = point[channel];
            }
        }
    }

    let axis = subtract(high, low);
    let axis_length_squared = dot(axis, axis);
    if axis_length_squared < MIN_AXIS_LENGTH_SQUARED {
        return Endpoints {
            first: low,
            second: high,
        };
    }

    let inverse_length_squared = 1.0 / axis_length_squared;
    let direction = axis.map(|component| component * inverse_length_squared);
    let middle = [
        (low[RED] + high[RED]) * 0.5,
        (low[GREEN] + high[GREEN]) * 0.5,
        (low[BLUE] + high[BLUE]) * 0.5,
    ];

    // The four box diagonals differ in the signs of green and blue relative to red.
    let mut diagonal_fit = [0.0f32; 4];
    for point in points {
        let along = [
            (point[RED] - middle[RED]) * direction[RED],
            (point[GREEN] - middle[GREEN]) * direction[GREEN],
            (point[BLUE] - middle[BLUE]) * direction[BLUE],
        ];
        let sums = [
            along[RED] + along[GREEN] + along[BLUE],
            along[RED] + along[GREEN] - along[BLUE],
            along[RED] - along[GREEN] + along[BLUE],
            along[RED] - along[GREEN] - along[BLUE],
        ];
        for (fit, sum) in diagonal_fit.iter_mut().zip(sums) {
            *fit += sum * sum;
        }
    }

    let mut best = 0;
    let mut best_fit = diagonal_fit[0];
    for (diagonal, &fit) in diagonal_fit.iter().enumerate().skip(1) {
        if fit > best_fit {
            best_fit = fit;
            best = diagonal;
        }
    }

    const FLIPS_GREEN: usize = 2;
    const FLIPS_BLUE: usize = 1;
    if best & FLIPS_GREEN != 0 {
        std::mem::swap(&mut low[GREEN], &mut high[GREEN]);
    }
    if best & FLIPS_BLUE != 0 {
        std::mem::swap(&mut low[BLUE], &mut high[BLUE]);
    }

    Endpoints {
        first: low,
        second: high,
    }
}

/// Newton iterations on the sum-of-squares error of the four-color palette. Boxes too small to
/// hold more than two colors keep their starting endpoints.
fn refine_endpoints(points: &[Rgb; TEXELS_PER_BLOCK], start: Endpoints) -> Endpoints {
    let Endpoints {
        first: mut low,
        second: mut high,
    } = start;

    let start_axis = subtract(high, low);
    if dot(start_axis, start_axis) < TWO_COLOR_AXIS_LENGTH_SQUARED {
        return Endpoints {
            first: low,
            second: high,
        };
    }

    let (first_end_weight, second_end_weight) = end_weights::<STEP_COUNT>(STEP_COUNT);
    for _ in 0..REFINE_ITERATIONS {
        let palette: [Rgb; STEP_COUNT] = std::array::from_fn(|step| {
            [RED, GREEN, BLUE].map(|channel| {
                low[channel] * first_end_weight[step] + high[channel] * second_end_weight[step]
            })
        });

        let mut direction = subtract(high, low);
        let length_squared = dot(direction, direction);
        if length_squared < MIN_REFINE_LENGTH_SQUARED {
            break;
        }
        let scale = LAST_POSITION / length_squared;
        direction = direction.map(|component| component * scale);

        let mut low_curvature = 0.0f32;
        let mut high_curvature = 0.0f32;
        let mut low_gradient = [0.0f32; 3];
        let mut high_gradient = [0.0f32; 3];

        for point in points {
            let position = dot(subtract(*point, low), direction);
            let step = nearest_step(position);
            let difference = subtract(palette[step], *point);

            let low_weight = first_end_weight[step] * WEIGHT_SCALE;
            let high_weight = second_end_weight[step] * WEIGHT_SCALE;

            low_curvature += low_weight * first_end_weight[step];
            high_curvature += high_weight * second_end_weight[step];
            for channel in 0..3 {
                low_gradient[channel] += low_weight * difference[channel];
                high_gradient[channel] += high_weight * difference[channel];
            }
        }

        if low_curvature > 0.0 {
            let move_scale = -1.0 / low_curvature;
            for channel in 0..3 {
                low[channel] += low_gradient[channel] * move_scale;
            }
        }
        if high_curvature > 0.0 {
            let move_scale = -1.0 / high_curvature;
            for channel in 0..3 {
                high[channel] += high_gradient[channel] * move_scale;
            }
        }

        let converged = low_gradient
            .iter()
            .chain(high_gradient.iter())
            .all(|gradient| gradient * gradient < REFINE_CONVERGED_SQUARED_STEP);
        if converged {
            break;
        }
    }

    Endpoints {
        first: low,
        second: high,
    }
}

/// The palette position nearest a point's projection onto the endpoint axis.
fn nearest_step(position: f32) -> usize {
    if position <= 0.0 {
        0
    } else if position >= LAST_POSITION {
        STEP_COUNT - 1
    } else {
        (position + ROUNDING) as u32 as usize
    }
}

/// Orders the quantized endpoints for four-color mode (first greater than second) and builds the
/// palette between them.
fn build_palette(first: u16, second: u16, weights: Rgb) -> Palette {
    let first_color = scale_rgb(decode_565(first), weights);
    let second_color = scale_rgb(decode_565(second), weights);

    let (stored_first, stored_second, start, end) = if first > second {
        (first, second, first_color, second_color)
    } else {
        (second, first, second_color, first_color)
    };

    let colors = [
        start,
        end,
        lerp(start, end, ONE_THIRD),
        lerp(start, end, TWO_THIRDS),
    ];
    Palette {
        stored_first,
        stored_second,
        colors,
    }
}

/// Picks each texel's palette index from its original color, diffusing the error when dithering.
fn assign_indices(
    texels: &[Texel; TEXELS_PER_BLOCK],
    palette: &Palette,
    weights: Rgb,
    dither: bool,
) -> u32 {
    let start = palette.colors[0];
    let axis = subtract(palette.colors[1], start);
    let scale = if palette.stored_first != palette.stored_second {
        LAST_POSITION / dot(axis, axis)
    } else {
        0.0
    };
    let direction = axis.map(|component| component * scale);

    let mut errors = [[0.0f32; 3]; TEXELS_PER_BLOCK];
    let mut indices = 0u32;
    for (index, texel) in texels.iter().enumerate() {
        let mut wanted = scale_rgb([texel[RED], texel[GREEN], texel[BLUE]], weights);
        if dither {
            for channel in 0..3 {
                wanted[channel] += errors[index][channel];
            }
        }

        let position = dot(subtract(wanted, start), direction);
        let code = if position <= 0.0 {
            0
        } else if position >= LAST_POSITION {
            1
        } else {
            POSITION_TO_CODE[(position + ROUNDING) as u32 as usize]
        };

        indices = ((code as u32) << INDEX_ENTRY_SHIFT) | (indices >> INDEX_BITS);

        if dither {
            let difference = subtract(wanted, palette.colors[code]);
            spread_error(index, |target, weight| {
                for channel in 0..3 {
                    errors[target][channel] += difference[channel] * weight;
                }
            });
        }
    }
    indices
}

/// Little-endian D3D layout: the two endpoints, then the index bitmap.
fn pack_color_block(first: u16, second: u16, indices: u32) -> [u8; COLOR_HALF_BYTES] {
    let mut block = [0u8; COLOR_HALF_BYTES];
    block[0..2].copy_from_slice(&first.to_le_bytes());
    block[2..4].copy_from_slice(&second.to_le_bytes());
    block[4..8].copy_from_slice(&indices.to_le_bytes());
    block
}

fn encode_565(color: Rgb) -> u16 {
    let [red, green, blue] = color.map(|channel| channel.clamp(0.0, 1.0));
    let red_bits = (red * RED_LEVELS + ROUNDING) as i32;
    let green_bits = (green * GREEN_LEVELS + ROUNDING) as i32;
    let blue_bits = (blue * BLUE_LEVELS + ROUNDING) as i32;
    ((red_bits << RED_SHIFT) | (green_bits << GREEN_SHIFT) | blue_bits) as u16
}

fn decode_565(packed: u16) -> Rgb {
    const RED_MASK: u16 = 0b1_1111;
    const GREEN_MASK: u16 = 0b11_1111;
    const BLUE_MASK: u16 = 0b1_1111;
    [
        f32::from((packed >> RED_SHIFT) & RED_MASK) * RED_LEVEL_STEP,
        f32::from((packed >> GREEN_SHIFT) & GREEN_MASK) * GREEN_LEVEL_STEP,
        f32::from(packed & BLUE_MASK) * BLUE_LEVEL_STEP,
    ]
}

fn scale_rgb(color: Rgb, weights: Rgb) -> Rgb {
    [
        color[RED] * weights[RED],
        color[GREEN] * weights[GREEN],
        color[BLUE] * weights[BLUE],
    ]
}

fn subtract(a: Rgb, b: Rgb) -> Rgb {
    [a[RED] - b[RED], a[GREEN] - b[GREEN], a[BLUE] - b[BLUE]]
}

fn dot(a: Rgb, b: Rgb) -> f32 {
    a[RED] * b[RED] + a[GREEN] * b[GREEN] + a[BLUE] * b[BLUE]
}

fn lerp(from: Rgb, to: Rgb, fraction: f32) -> Rgb {
    [RED, GREEN, BLUE].map(|channel| from[channel] + fraction * (to[channel] - from[channel]))
}
