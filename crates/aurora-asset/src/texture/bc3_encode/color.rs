//! The color half of a BC3 block: two RGB565 endpoints and sixteen 2-bit indices (BC1 four-color
//! mode). Endpoints are searched in a perceptually weighted color space.

use super::x87::{Fpu, X87};
use super::{Bc3Options, TEXELS_PER_BLOCK, Texel, diffuse_error, end_weights, spread_error};

mod underrun;

const COLOR_HALF_BYTES: usize = 8;
const INDEX_BITS: u32 = 2;
const INDEX_ENTRY_SHIFT: u32 = 32 - INDEX_BITS;

type Rgb = [f32; 3];
const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;
const CHANNELS: [usize; 3] = [RED, GREEN, BLUE];

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

// Derived from AuroraAsset.dll for interoperability: these loops run with the FPU truncating.
const PREPARE_FPU: Fpu = Fpu::RoundTowardZero;
const REFINE_FPU: Fpu = Fpu::RoundTowardZero;
const QUANTIZE_FPU: Fpu = Fpu::RoundTowardZero;
const INDEX_FPU: Fpu = Fpu::RoundTowardZero;
// Derived from AuroraAsset.dll for interoperability: the endpoint search setup and the index scale round to nearest.
const SEARCH_FPU: Fpu = Fpu::RoundToNearest;
const INDEX_SCALE_FPU: Fpu = Fpu::RoundToNearest;

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
    let weights = LUMINANCE;
    let inverse_weights = LUMINANCE_INVERSE;

    let points = prepare_block(texels, weights, options.dither);
    let chosen = choose_endpoints(&points, weights);
    let endpoints = refine_endpoints(&points, chosen);

    let first = encode_565(scale_rgb(endpoints.first, inverse_weights));
    let second = encode_565(scale_rgb(endpoints.second, inverse_weights));
    if first == second {
        return pack_color_block(first, second, 0);
    }

    let palette = build_palette(first, second, weights);
    let indices = assign_indices(texels, &palette, weights, options.dither);
    pack_color_block(palette.stored_first, palette.stored_second, indices)
}

/// Rounds the block's colors to RGB565 levels, diffusing the rounding error when dithering, and
/// moves them into the weighted space the endpoint search runs in.
fn prepare_block(
    texels: &[Texel; TEXELS_PER_BLOCK],
    weights: Rgb,
    dither: bool,
) -> [Rgb; TEXELS_PER_BLOCK] {
    let fpu = PREPARE_FPU;
    let mut points = [[0.0f32; 3]; TEXELS_PER_BLOCK];
    let mut errors = [[0.0f32; 3]; TEXELS_PER_BLOCK];

    for (index, texel) in texels.iter().enumerate() {
        let mut wanted = [texel[RED], texel[GREEN], texel[BLUE]];
        if dither {
            for channel in CHANNELS {
                wanted[channel] = (fpu.load(errors[index][channel]) + wanted[channel]).store();
            }
        }

        let rounded = [
            round_to_level(wanted[RED], RED_LEVELS, RED_LEVEL_STEP, fpu),
            round_to_level(wanted[GREEN], GREEN_LEVELS, GREEN_LEVEL_STEP, fpu),
            round_to_level(wanted[BLUE], BLUE_LEVELS, BLUE_LEVEL_STEP, fpu),
        ];

        if dither {
            let difference =
                CHANNELS.map(|channel| (fpu.load(wanted[channel]) - rounded[channel]).store());
            spread_error(index, |target, weight| {
                for channel in CHANNELS {
                    errors[target][channel] =
                        diffuse_error(errors[target][channel], difference[channel], weight);
                }
            });
        }

        points[index] =
            CHANNELS.map(|channel| (fpu.load(rounded[channel]) * weights[channel]).store());
    }
    points
}

/// Rounds a channel to the nearest of `levels + 1` evenly spaced values.
fn round_to_level(value: f32, levels: f32, level_step: f32, fpu: Fpu) -> f32 {
    let level = (fpu.load(value) * levels + ROUNDING).store() as i32;
    (fpu.load(level as f32) * level_step).store()
}

/// Starting endpoints: the corners of the points' bounding box, choosing the box diagonal that
/// the points follow most closely.
fn choose_endpoints(points: &[Rgb; TEXELS_PER_BLOCK], weights: Rgb) -> Endpoints {
    let mut low = weights;
    let mut high = [0.0f32; 3];
    for point in points {
        for channel in CHANNELS {
            if point[channel] < low[channel] {
                low[channel] = point[channel];
            }
            if point[channel] > high[channel] {
                high[channel] = point[channel];
            }
        }
    }

    let axis = subtract(high, low);
    let fpu = SEARCH_FPU;
    let axis_length_squared = wide_dot(axis, axis, fpu).store();
    if axis_length_squared < MIN_AXIS_LENGTH_SQUARED {
        return Endpoints {
            first: low,
            second: high,
        };
    }

    let inverse_length_squared = 1.0 / axis_length_squared;
    let direction = axis.map(|component| component * inverse_length_squared);
    let middle = CHANNELS.map(|channel| (low[channel] + high[channel]) * 0.5);

    // The four box diagonals differ in the signs of green and blue relative to red.
    let mut diagonal_fit = [0.0f32; 4];
    for point in points {
        let [red, green, blue] = CHANNELS
            .map(|channel| {
                ((fpu.load(point[channel]) - middle[channel]) * direction[channel]).store()
            })
            .map(|along| fpu.load(along));
        // Derived from AuroraAsset.dll for interoperability: green is added to red before blue comes in.
        let sums = [
            green + red + blue,
            green + red - blue,
            red - green + blue,
            red - green - blue,
        ];
        for (fit, sum) in diagonal_fit.iter_mut().zip(sums) {
            let sum = sum.store();
            *fit = (fpu.load(sum) * sum + *fit).store();
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
    let start_length_squared = wide_dot(start_axis, start_axis, SEARCH_FPU).store();
    if start_length_squared < TWO_COLOR_AXIS_LENGTH_SQUARED {
        return Endpoints {
            first: low,
            second: high,
        };
    }

    let fpu = REFINE_FPU;
    let (first_end_weight, second_end_weight) = end_weights::<STEP_COUNT>(STEP_COUNT);
    for iteration in 0..REFINE_ITERATIONS {
        let palette: [Rgb; STEP_COUNT] = std::array::from_fn(|step| {
            CHANNELS.map(|channel| {
                // Derived from AuroraAsset.dll for interoperability: the high endpoint's share comes first.
                let from_high = fpu.load(second_end_weight[step]) * high[channel];
                (from_high + fpu.load(first_end_weight[step]) * low[channel]).store()
            })
        });

        let mut direction =
            CHANNELS.map(|channel| (fpu.load(high[channel]) - low[channel]).store());
        let length_squared = wide_dot(direction, direction, fpu).store();
        if length_squared < MIN_REFINE_LENGTH_SQUARED {
            break;
        }
        let scale = (fpu.load(LAST_POSITION) / length_squared).store();
        direction = direction.map(|component| (fpu.load(scale) * component).store());

        let palette_step = |step: usize| StepRead {
            color: palette[step],
            low_weight: first_end_weight[step],
            high_weight: second_end_weight[step],
        };
        let mut sums = NewtonSums::default();
        for point in points {
            let position = projection(*point, low, direction, fpu);
            let step = match refine_step(position, fpu) {
                Ok(step) => palette_step(step),
                // Deeper steps read stack residue or load addresses in the DLL; step 0 stands in.
                Err(step) => underrun::read_before_palette(step, &low, &high, &sums, iteration)
                    .unwrap_or_else(|| palette_step(0)),
            };
            sums.add(*point, &step, fpu);
        }

        if sums.low_curvature > 0.0 {
            move_endpoint(&mut low, sums.low_gradient, sums.low_curvature, fpu);
        }
        if sums.high_curvature > 0.0 {
            move_endpoint(&mut high, sums.high_gradient, sums.high_curvature, fpu);
        }

        let converged = sums
            .low_gradient
            .iter()
            .chain(sums.high_gradient.iter())
            .all(|&gradient| fpu.load(gradient) * gradient < REFINE_CONVERGED_SQUARED_STEP);
        if converged {
            break;
        }
    }

    Endpoints {
        first: low,
        second: high,
    }
}

/// The palette entry a point is compared with and how much each endpoint contributes to it.
struct StepRead {
    color: Rgb,
    low_weight: f32,
    high_weight: f32,
}

/// The gradient and curvature of the squared error, per endpoint, summed over the points.
#[derive(Default)]
struct NewtonSums {
    low_curvature: f32,
    high_curvature: f32,
    low_gradient: Rgb,
    high_gradient: Rgb,
}

impl NewtonSums {
    fn add(&mut self, point: Rgb, step: &StepRead, fpu: Fpu) {
        let difference =
            CHANNELS.map(|channel| (fpu.load(step.color[channel]) - point[channel]).store());
        let low_share = (fpu.load(step.low_weight) * WEIGHT_SCALE).store();
        let high_share = (fpu.load(step.high_weight) * WEIGHT_SCALE).store();

        self.low_curvature = (fpu.load(low_share) * step.low_weight + self.low_curvature).store();
        self.high_curvature =
            (fpu.load(high_share) * step.high_weight + self.high_curvature).store();
        for channel in CHANNELS {
            self.low_gradient[channel] =
                (fpu.load(low_share) * difference[channel] + self.low_gradient[channel]).store();
            self.high_gradient[channel] =
                (fpu.load(high_share) * difference[channel] + self.high_gradient[channel]).store();
        }
    }
}

/// One Newton step: moves the endpoint against its gradient, scaled by the curvature.
fn move_endpoint(endpoint: &mut Rgb, gradient: Rgb, curvature: f32, fpu: Fpu) {
    let move_scale = (fpu.load(-1.0) / curvature).store();
    for channel in CHANNELS {
        endpoint[channel] = (fpu.load(move_scale) * gradient[channel] + endpoint[channel]).store();
    }
}

/// The palette position nearest a point's projection onto the endpoint axis, or the negative
/// step a point far before the low endpoint rounds to.
// Derived from AuroraAsset.dll for interoperability: there is no lower bound on the step here.
fn refine_step(position: f32, fpu: Fpu) -> Result<usize, i32> {
    if position >= LAST_POSITION {
        Ok(STEP_COUNT - 1)
    } else {
        let step = (fpu.load(position) + ROUNDING).store() as i32;
        usize::try_from(step).map_err(|_| step)
    }
}

/// `(point - origin) . direction`, with the differences kept on the FPU stack.
// Derived from AuroraAsset.dll for interoperability: the terms are summed green, red, blue.
fn projection(point: Rgb, origin: Rgb, direction: Rgb, fpu: Fpu) -> f32 {
    let along = |channel: usize| (fpu.load(point[channel]) - origin[channel]) * direction[channel];
    (along(GREEN) + along(RED) + along(BLUE)).store()
}

/// `a . b` evaluated on the FPU stack.
fn wide_dot(a: Rgb, b: Rgb, fpu: Fpu) -> X87 {
    let along = |channel: usize| fpu.load(a[channel]) * b[channel];
    along(RED) + along(GREEN) + along(BLUE)
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
    let fpu = INDEX_FPU;
    let start = palette.colors[0];
    let axis = subtract(palette.colors[1], start);
    let scale = if palette.stored_first != palette.stored_second {
        // Derived from AuroraAsset.dll for interoperability: one expression, with no store between the dot and the division.
        let scale_fpu = INDEX_SCALE_FPU;
        (scale_fpu.load(LAST_POSITION) / wide_dot(axis, axis, scale_fpu)).store()
    } else {
        0.0
    };
    let direction = axis.map(|component| component * scale);

    let mut errors = [[0.0f32; 3]; TEXELS_PER_BLOCK];
    let mut indices = 0u32;
    for (index, texel) in texels.iter().enumerate() {
        let mut wanted =
            CHANNELS.map(|channel| (fpu.load(texel[channel]) * weights[channel]).store());
        if dither {
            for channel in CHANNELS {
                wanted[channel] = (fpu.load(errors[index][channel]) + wanted[channel]).store();
            }
        }

        let position = projection(wanted, start, direction, fpu);
        let code = if position <= 0.0 {
            0
        } else if position >= LAST_POSITION {
            1
        } else {
            POSITION_TO_CODE[(fpu.load(position) + ROUNDING).store() as u32 as usize]
        };

        indices = ((code as u32) << INDEX_ENTRY_SHIFT) | (indices >> INDEX_BITS);

        if dither {
            let difference = CHANNELS
                .map(|channel| (fpu.load(wanted[channel]) - palette.colors[code][channel]).store());
            spread_error(index, |target, weight| {
                for channel in CHANNELS {
                    errors[target][channel] =
                        diffuse_error(errors[target][channel], difference[channel], weight);
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
    let fpu = QUANTIZE_FPU;
    let [red, green, blue] = color.map(|channel| channel.clamp(0.0, 1.0));
    let red_bits = (fpu.load(red) * RED_LEVELS + ROUNDING).store() as i32;
    let green_bits = (fpu.load(green) * GREEN_LEVELS + ROUNDING).store() as i32;
    let blue_bits = (fpu.load(blue) * BLUE_LEVELS + ROUNDING).store() as i32;
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

fn lerp(from: Rgb, to: Rgb, fraction: f32) -> Rgb {
    [RED, GREEN, BLUE].map(|channel| from[channel] + fraction * (to[channel] - from[channel]))
}
