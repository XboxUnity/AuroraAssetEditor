//! Deterministic synthetic test images for the golden corpus.
//!
//! The pixel formulas follow `tools/golden/spike/gen_inputs.py`; the random values come from
//! an in-repo PRNG so the corpus never depends on an external crate's generator.

use image::{Rgba, RgbaImage};

const PRNG_SEED: u64 = 1234;
const PRNG_MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;
const XORSHIFT_RIGHT_FIRST: u32 = 12;
const XORSHIFT_LEFT: u32 = 25;
const XORSHIFT_RIGHT_SECOND: u32 = 27;
const TOP_BYTE_SHIFT: u32 = 56;

const OPAQUE: u8 = 255;
const SYNTHETIC_SIZE: u32 = 256;
const NEAR_EXTREME_RANGE: u8 = 8;
const BLOCK_SIDE: u32 = 4;
const BLOCK_TEXELS: usize = (BLOCK_SIDE * BLOCK_SIDE) as usize;
const TWO_TONE_CELL_PIXELS: u32 = 4;
const TWO_TONE_FIRST: [u8; 4] = [0x20, 0x60, 0xA0, OPAQUE];
const TWO_TONE_SECOND: [u8; 4] = [0xE0, 0x90, 0x10, OPAQUE];
const SOLID_MID: [u8; 4] = [0x80, 0x40, 0xC0, OPAQUE];

const ODD_SIZES: [(&str, u32, u32); 5] = [
    ("odd_1x1", 1, 1),
    ("odd_3x5", 3, 5),
    ("odd_5x3", 5, 3),
    ("odd_63x65", 63, 65),
    ("odd_130x70", 130, 70),
];

/// xorshift64* generator.
pub struct Prng {
    state: u64,
}

impl Prng {
    pub fn new() -> Self {
        Self::with_seed(PRNG_SEED)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> XORSHIFT_RIGHT_FIRST;
        self.state ^= self.state << XORSHIFT_LEFT;
        self.state ^= self.state >> XORSHIFT_RIGHT_SECOND;
        self.state.wrapping_mul(PRNG_MULTIPLIER)
    }

    pub fn next_byte(&mut self) -> u8 {
        (self.next_u64() >> TOP_BYTE_SHIFT) as u8
    }

    fn next_byte_below(&mut self, limit: u8) -> u8 {
        self.next_byte() % limit
    }
}

fn square_image(pixel_at: impl FnMut(u32, u32) -> [u8; 4]) -> RgbaImage {
    image_of_size(SYNTHETIC_SIZE, SYNTHETIC_SIZE, pixel_at)
}

fn image_of_size(
    width: u32,
    height: u32,
    mut pixel_at: impl FnMut(u32, u32) -> [u8; 4],
) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| Rgba(pixel_at(x, y)))
}

fn random_rgb(prng: &mut Prng, alpha: u8) -> [u8; 4] {
    [prng.next_byte(), prng.next_byte(), prng.next_byte(), alpha]
}

fn random_rgba(prng: &mut Prng) -> [u8; 4] {
    [
        prng.next_byte(),
        prng.next_byte(),
        prng.next_byte(),
        prng.next_byte(),
    ]
}

fn gradient() -> RgbaImage {
    square_image(|x, y| [x as u8, y as u8, ((x + y) / 2) as u8, OPAQUE])
}

fn noise(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| random_rgb(prng, OPAQUE))
}

fn sine_channel(coordinate: u32, period: f64) -> u8 {
    (127.0 + 127.0 * (f64::from(coordinate) / period).sin()) as u8
}

fn sines() -> RgbaImage {
    square_image(|x, y| {
        [
            sine_channel(x, 9.0),
            sine_channel(y, 13.0),
            sine_channel(x + y, 21.0),
            OPAQUE,
        ]
    })
}

fn alpha_gradient() -> RgbaImage {
    square_image(|x, y| [x as u8, y as u8, OPAQUE - x as u8, ((x + y) / 2) as u8])
}

fn alpha_noise(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| random_rgba(prng))
}

fn solid_mid() -> RgbaImage {
    square_image(|_, _| SOLID_MID)
}

fn two_tone() -> RgbaImage {
    square_image(|x, y| {
        if (x / TWO_TONE_CELL_PIXELS + y / TWO_TONE_CELL_PIXELS).is_multiple_of(2) {
            TWO_TONE_FIRST
        } else {
            TWO_TONE_SECOND
        }
    })
}

fn near_black(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| {
        [
            prng.next_byte_below(NEAR_EXTREME_RANGE),
            prng.next_byte_below(NEAR_EXTREME_RANGE),
            prng.next_byte_below(NEAR_EXTREME_RANGE),
            OPAQUE,
        ]
    })
}

fn near_white(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| {
        [
            OPAQUE - prng.next_byte_below(NEAR_EXTREME_RANGE),
            OPAQUE - prng.next_byte_below(NEAR_EXTREME_RANGE),
            OPAQUE - prng.next_byte_below(NEAR_EXTREME_RANGE),
            OPAQUE,
        ]
    })
}

fn alpha_zero(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| random_rgb(prng, 0))
}

fn alpha_binary(prng: &mut Prng) -> RgbaImage {
    square_image(|_, _| {
        let alpha = if prng.next_byte().is_multiple_of(2) {
            0
        } else {
            OPAQUE
        };
        random_rgb(prng, alpha)
    })
}

fn checker_1px() -> RgbaImage {
    square_image(|x, y| {
        if (x + y).is_multiple_of(2) {
            [0, 0, 0, OPAQUE]
        } else {
            [OPAQUE, OPAQUE, OPAQUE, OPAQUE]
        }
    })
}

/// One opaque block whose colour endpoint optimizer sees a point at step -1, before its palette
/// (see docs/codec/xg-bc3-encoder.md). Found by a hill-climbing search.
const COLOR_UNDERRUN_STEP_1_BLOCK: [[u8; 4]; BLOCK_TEXELS] = [
    [30, 248, 96, OPAQUE],
    [171, 249, 18, OPAQUE],
    [28, 245, 93, OPAQUE],
    [56, 227, 68, OPAQUE],
    [35, 227, 75, OPAQUE],
    [29, 248, 53, OPAQUE],
    [28, 230, 81, OPAQUE],
    [39, 240, 86, OPAQUE],
    [27, 246, 59, OPAQUE],
    [148, 247, 18, OPAQUE],
    [28, 245, 46, OPAQUE],
    [41, 230, 15, OPAQUE],
    [29, 248, 42, OPAQUE],
    [132, 246, 0, OPAQUE],
    [0, 212, 194, OPAQUE],
    [37, 238, 103, OPAQUE],
];
fn single_block(texels: &[[u8; 4]; BLOCK_TEXELS]) -> RgbaImage {
    image_of_size(BLOCK_SIDE, BLOCK_SIDE, |x, y| {
        texels[(y * BLOCK_SIDE + x) as usize]
    })
}

fn odd_size_noise(prng: &mut Prng, width: u32, height: u32) -> RgbaImage {
    image_of_size(width, height, |_, _| random_rgba(prng))
}

/// Every synthetic corpus image with its fixture name, in a fixed order.
pub fn all_synthetic_images() -> Vec<(String, RgbaImage)> {
    let mut prng = Prng::new();
    let mut images = vec![
        ("grad".to_string(), gradient()),
        ("noise".to_string(), noise(&mut prng)),
        ("sines".to_string(), sines()),
        ("alphagrad".to_string(), alpha_gradient()),
        ("alphanoise".to_string(), alpha_noise(&mut prng)),
        ("solid_mid".to_string(), solid_mid()),
        ("two_tone".to_string(), two_tone()),
        ("near_black".to_string(), near_black(&mut prng)),
        ("near_white".to_string(), near_white(&mut prng)),
        ("alpha_zero".to_string(), alpha_zero(&mut prng)),
        ("alpha_binary".to_string(), alpha_binary(&mut prng)),
        ("checker_1px".to_string(), checker_1px()),
    ];
    for (name, width, height) in ODD_SIZES {
        images.push((name.to_string(), odd_size_noise(&mut prng, width, height)));
    }
    images.push((
        "color_underrun_step_1".to_string(),
        single_block(&COLOR_UNDERRUN_STEP_1_BLOCK),
    ));
    images
}
