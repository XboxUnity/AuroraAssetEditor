use super::HEADER_BYTE_LENGTH;
use crate::minimum_read::SurfaceUnit;
use crate::prng::Prng;

const DWORD_BYTES: usize = 4;
const PRNG_SEED_BASE: u64 = 0x5EED_0000;
const VIDEO_SEED_BLOCK_STRIDE: u64 = 10_000;

const FETCH_0_DWORD: usize = 7;
const FETCH_1_DWORD: usize = 8;
const FETCH_3_DWORD: usize = 10;

const TILED_SHIFT: u32 = 31;
const PITCH_SHIFT: u32 = 22;
const PITCH_BITS: u32 = 9;
const SIGN_SHIFT: u32 = 2;
const SIGN_BITS: u32 = 8;
const DATA_FORMAT_SHIFT: u32 = 0;
const DATA_FORMAT_BITS: u32 = 6;
const ENDIAN_SHIFT: u32 = 6;
const ENDIAN_BITS: u32 = 2;
const NUM_FORMAT_SHIFT: u32 = 0;
const NUM_FORMAT_BITS: u32 = 1;
const SWIZZLE_SHIFTS: [u32; 4] = [1, 4, 7, 10];
const SWIZZLE_BITS: u32 = 3;

const GPU_FORMAT_DXT1: u32 = 0x12;
const GPU_FORMAT_DXT2_3: u32 = 0x13;
const GPU_FORMAT_DXT4_5: u32 = 0x14;

const SIGN_ALL_UNSIGNED: u32 = 0b0000_0000;
const SIGN_ALL_SIGNED: u32 = 0b0101_0101;
const SIGN_ALL_GAMMA: u32 = 0b1111_1111;

const BLOCKS_PER_ROW_ALIGNMENT: usize = 32;
const BLOCK_ROWS_ALIGNMENT: usize = 32;
const BLOCK_SIDE_TEXELS: usize = 4;
const DXT1_BLOCK_BYTES: usize = 8;
const BC3_BLOCK_BYTES: usize = 16;
const BC3_COLOR_HALF_OFFSET: usize = 8;
const COLOR_ENDPOINT_BYTES: usize = 2;

const SMALL_SIZE: (u32, u32) = (32, 32);
const ODD_SIZE: (u32, u32) = (130, 70);
const ENDIANS: [u32; 4] = [0, 1, 2, 3];
const RGBA: (&str, [u32; 4]) = ("rgba", [0, 1, 2, 3]);
const BGRA: (&str, [u32; 4]) = ("bgra", [2, 1, 0, 3]);
const DXT1: (&str, u32) = ("dxt1", GPU_FORMAT_DXT1);
const DXT2_3: (&str, u32) = ("dxt2_3", GPU_FORMAT_DXT2_3);
const DXT4_5: (&str, u32) = ("dxt4_5", GPU_FORMAT_DXT4_5);
const PITCH_UNITS_WIDER_THAN_NEEDED: u32 = 12;
const ARGB_PITCH_UNITS_WIDER_THAN_NEEDED: u32 = 4;
const TINY_SIZE: (u32, u32) = (8, 8);
const ARGB_ODD_SIZE: (u32, u32) = (40, 24);
const GPU_FORMAT_8_8_8_8: u32 = 0x06;
const ARGB_BYTES: usize = 4;
const TEXELS_PER_PITCH_UNIT: usize = 32;
const BLOCK_ROW_PITCH_ALIGNMENT_BYTES: usize = 256;
const ARGB_ENDIAN: u32 = 2;
const ARGB_SWIZZLE: (&str, [u32; 4]) = ("bgra", [2, 1, 0, 3]);

/// Header bits the port does not read, flipped together to prove they do not change the DLL's
/// decode: (label, [(dword, xor mask)]). The one group that does change it is `border`.
const IGNORED_FIELD_PROBES: [(&str, &[(usize, u32)]); 8] = [
    ("common_flags", &[(0, 0xFFFF_FFF0)]),
    (
        "gpu_runtime",
        &[
            (1, u32::MAX),
            (2, u32::MAX),
            (3, u32::MAX),
            (4, u32::MAX),
            (5, u32::MAX),
            (6, u32::MAX),
        ],
    ),
    ("fetch0_misc", &[(7, 0x003F_FC03)]),
    ("fetch1_misc", &[(8, 0xFFFF_FF00)]),
    ("fetch2_depth", &[(9, 0xFC00_0000)]),
    ("fetch3_filters", &[(10, 0x7FFF_E000)]),
    ("fetch4", &[(11, u32::MAX)]),
    ("fetch5_misc", &[(12, 0xFFFF_F9FF)]),
];
/// FETCH_3 bit 31, which changes the DLL's decode, so the port requires it to be clear.
const BORDER_PROBE: (&str, &[(usize, u32)]) = ("border", &[(10, 0x8000_0000)]);

const SWIZZLE_SELECTORS: [u32; 4] = [0, 1, 2, 3];
const SWIZZLE_CONSTANT_ZERO: u32 = 4;

#[derive(Clone, Copy)]
enum VideoPattern {
    Random,
    /// Both colour endpoints of every block are equal, probing the "first greater than second"
    /// test.
    EqualColorEndpoints,
}

/// How a surface is divided into the units its rows are made of.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Surface {
    Bc1,
    Bc3,
    Argb,
}

impl Surface {
    fn texels_per_side(self) -> usize {
        match self {
            Self::Bc1 | Self::Bc3 => BLOCK_SIDE_TEXELS,
            Self::Argb => 1,
        }
    }

    fn unit_bytes(self) -> usize {
        match self {
            Self::Bc1 => DXT1_BLOCK_BYTES,
            Self::Bc3 => BC3_BLOCK_BYTES,
            Self::Argb => ARGB_BYTES,
        }
    }

    pub(super) fn unit(self) -> SurfaceUnit {
        SurfaceUnit {
            texels_per_side: self.texels_per_side(),
            bytes: self.unit_bytes(),
        }
    }

    fn video_name_part(self) -> &'static str {
        match self {
            Self::Bc1 => "bc1",
            Self::Bc3 => "bc3",
            Self::Argb => "argb",
        }
    }
}

#[derive(Clone)]
pub(super) struct Case {
    pub(super) name: String,
    pub(super) surface: Surface,
    data_format: u32,
    endian: u32,
    swizzle: [u32; 4],
    num_format: u32,
    sign: u32,
    tiled: bool,
    pitch_units: Option<u32>,
    flips: Vec<(usize, u32)>,
    /// The case whose decode this one is compared with: equal output is stored as a reference.
    pub(super) baseline: Option<String>,
    pub(super) width: u32,
    pub(super) height: u32,
    pattern: VideoPattern,
}

impl Case {
    fn new(
        (format_name, data_format): (&str, u32),
        endian: u32,
        (swizzle_name, swizzle): (&str, [u32; 4]),
        (width, height): (u32, u32),
    ) -> Self {
        let surface = if data_format == GPU_FORMAT_DXT1 {
            Surface::Bc1
        } else {
            Surface::Bc3
        };
        Self {
            name: format!("{format_name}_e{endian}_{swizzle_name}_{width}x{height}"),
            surface,
            data_format,
            endian,
            swizzle,
            num_format: 0,
            sign: SIGN_ALL_UNSIGNED,
            tiled: false,
            pitch_units: None,
            flips: Vec::new(),
            baseline: None,
            width,
            height,
            pattern: VideoPattern::Random,
        }
    }

    /// The DLL's own ARGB8888 header.
    fn argb(size: (u32, u32)) -> Self {
        let mut case = Self::new(
            ("argb", GPU_FORMAT_8_8_8_8),
            ARGB_ENDIAN,
            ARGB_SWIZZLE,
            size,
        );
        case.surface = Surface::Argb;
        case
    }

    /// Inserts `label` before the size suffix of the name.
    fn labelled(mut self, label: &str) -> Self {
        let size = format!("_{}x{}", self.width, self.height);
        self.name = self.name.replace(&size, &format!("_{label}{size}"));
        self
    }

    fn with_num_format(mut self) -> Self {
        self.num_format = 1;
        self.labelled("n1")
    }

    fn with_sign(mut self, sign: u32, label: &str) -> Self {
        self.sign = sign;
        self.labelled(label)
    }

    fn tiled(mut self) -> Self {
        self.tiled = true;
        self.labelled("tiled")
    }

    fn with_pitch_units(mut self, pitch_units: u32) -> Self {
        self.pitch_units = Some(pitch_units);
        self.labelled(&format!("pitch{pitch_units}"))
    }

    fn with_equal_color_endpoints(mut self) -> Self {
        self.pattern = VideoPattern::EqualColorEndpoints;
        self.labelled("equal_endpoints")
    }

    fn with_swizzle(mut self, swizzle: [u32; 4]) -> Self {
        let digits: String = swizzle.iter().map(u32::to_string).collect();
        self.swizzle = swizzle;
        self.name = self.name.replace("_rgba_", &format!("_swz{digits}_"));
        self
    }

    /// Flips header bits and compares the decode with the unflipped case.
    fn flipped(mut self, (label, flips): (&str, &[(usize, u32)])) -> Self {
        self.baseline = Some(self.name.clone());
        self.flips = flips.to_vec();
        self.labelled(&format!("probe_{label}"))
    }

    fn row_pitch(&self) -> usize {
        let unit_bytes = self.surface.unit_bytes();
        match self.pitch_units {
            Some(units) => {
                let units_per_pitch_unit = TEXELS_PER_PITCH_UNIT / self.surface.texels_per_side();
                units as usize * units_per_pitch_unit * unit_bytes
            }
            None => {
                let units_wide = (self.width as usize).div_ceil(self.surface.texels_per_side());
                let row_bytes = align(units_wide, BLOCKS_PER_ROW_ALIGNMENT) * unit_bytes;
                align(row_bytes, BLOCK_ROW_PITCH_ALIGNMENT_BYTES)
            }
        }
    }

    /// The zero-padded surface size the DLL needs for this header.
    fn video_len(&self) -> usize {
        let rows = (self.height as usize).div_ceil(self.surface.texels_per_side());
        self.row_pitch() * align(rows, BLOCK_ROWS_ALIGNMENT)
    }

    /// Cases with the same bytes share one video file.
    pub(super) fn video_file_name(&self) -> String {
        let pattern = match self.pattern {
            VideoPattern::Random => "random",
            VideoPattern::EqualColorEndpoints => "equal_endpoints",
        };
        let pitch = match self.pitch_units {
            Some(units) => format!("_pitch{units}"),
            None => String::new(),
        };
        format!(
            "{pattern}_{}_{}x{}{pitch}.vid",
            self.surface.video_name_part(),
            self.width,
            self.height
        )
    }

    /// Cases of one unit size and width share their random bytes, so two cases whose headers
    /// differ in a probed field decode identically exactly when that field does not matter.
    fn video_seed(&self) -> u64 {
        PRNG_SEED_BASE
            + self.surface.unit_bytes() as u64 * VIDEO_SEED_BLOCK_STRIDE
            + u64::from(self.width)
    }

    pub(super) fn video(&self) -> Vec<u8> {
        let mut prng = Prng::with_seed(self.video_seed());
        let mut video: Vec<u8> = (0..self.video_len()).map(|_| prng.next_byte()).collect();
        match self.pattern {
            VideoPattern::Random => {}
            VideoPattern::EqualColorEndpoints => self.equalise_color_endpoints(&mut video),
        }
        video
    }

    fn equalise_color_endpoints(&self, video: &mut [u8]) {
        let color_offset = match self.surface {
            Surface::Bc3 => BC3_COLOR_HALF_OFFSET,
            _ => 0,
        };
        for block in video.chunks_exact_mut(self.surface.unit_bytes()) {
            let first = color_offset..color_offset + COLOR_ENDPOINT_BYTES;
            let second_start = first.end;
            block.copy_within(first, second_start);
        }
    }
}

fn swizzle_permutations() -> Vec<[u32; 4]> {
    let mut permutations = Vec::new();
    for first in SWIZZLE_SELECTORS {
        for second in SWIZZLE_SELECTORS {
            for third in SWIZZLE_SELECTORS {
                for fourth in SWIZZLE_SELECTORS {
                    let candidate = [first, second, third, fourth];
                    if SWIZZLE_SELECTORS.iter().all(|s| candidate.contains(s)) {
                        permutations.push(candidate);
                    }
                }
            }
        }
    }
    permutations
}

pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (size, endians) in [(SMALL_SIZE, ENDIANS.as_slice()), (ODD_SIZE, [1].as_slice())] {
        for format in [DXT1, DXT2_3, DXT4_5] {
            for endian in endians.iter().copied() {
                for swizzle in [RGBA, BGRA] {
                    cases.push(Case::new(format, endian, swizzle, size));
                }
            }
        }
    }
    for format in [DXT1, DXT2_3, DXT4_5] {
        let aurora_like = || Case::new(format, 1, RGBA, SMALL_SIZE);
        cases.push(aurora_like().tiled());
        cases.push(aurora_like().with_num_format());
        cases.push(aurora_like().with_sign(SIGN_ALL_SIGNED, "signed"));
        cases.push(aurora_like().with_sign(SIGN_ALL_GAMMA, "gamma"));
    }
    for format in [DXT1, DXT4_5] {
        cases.push(
            Case::new(format, 1, RGBA, ODD_SIZE).with_pitch_units(PITCH_UNITS_WIDER_THAN_NEEDED),
        );
        cases.push(Case::new(format, 1, RGBA, SMALL_SIZE).with_equal_color_endpoints());
    }
    cases.push(Case::argb(ARGB_ODD_SIZE));
    cases.push(Case::argb(ARGB_ODD_SIZE).with_pitch_units(ARGB_PITCH_UNITS_WIDER_THAN_NEEDED));

    for format in [DXT1, DXT4_5] {
        cases.push(Case::new(format, 1, RGBA, TINY_SIZE));
        for permutation in swizzle_permutations() {
            if permutation != RGBA.1 {
                cases.push(Case::new(format, 1, RGBA, TINY_SIZE).with_swizzle(permutation));
            }
        }
        for selectors in [[0, 0, 0, 3], [3, 3, 3, 3]] {
            cases.push(Case::new(format, 1, RGBA, TINY_SIZE).with_swizzle(selectors));
        }
        let constant_alpha = [0, 1, 2, SWIZZLE_CONSTANT_ZERO];
        cases.push(Case::new(format, 1, RGBA, TINY_SIZE).with_swizzle(constant_alpha));
    }

    let baselines = [
        Case::new(DXT4_5, 1, RGBA, TINY_SIZE),
        Case::new(DXT1, 1, RGBA, TINY_SIZE),
        Case::argb(TINY_SIZE),
    ];
    for baseline in baselines {
        if baseline.surface == Surface::Argb {
            cases.push(baseline.clone());
        }
        for probe in IGNORED_FIELD_PROBES.into_iter().chain([BORDER_PROBE]) {
            cases.push(baseline.clone().flipped(probe));
        }
    }
    cases
}

fn align(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn set_bits(word: u32, shift: u32, bits: u32, value: u32) -> u32 {
    let mask = (1u32 << bits) - 1;
    (word & !(mask << shift)) | ((value & mask) << shift)
}

fn patch_dword(header: &mut [u8; HEADER_BYTE_LENGTH], index: usize, patch: impl Fn(u32) -> u32) {
    let range = index * DWORD_BYTES..(index + 1) * DWORD_BYTES;
    let mut word_bytes = [0u8; DWORD_BYTES];
    word_bytes.copy_from_slice(&header[range.clone()]);
    let word = u32::from_be_bytes(word_bytes);
    header[range].copy_from_slice(&patch(word).to_be_bytes());
}

pub(super) fn crafted_header(
    base: [u8; HEADER_BYTE_LENGTH],
    case: &Case,
) -> [u8; HEADER_BYTE_LENGTH] {
    let mut header = base;
    patch_dword(&mut header, FETCH_0_DWORD, |word| {
        let word = set_bits(word, TILED_SHIFT, 1, u32::from(case.tiled));
        let word = set_bits(word, SIGN_SHIFT, SIGN_BITS, case.sign);
        match case.pitch_units {
            Some(units) => set_bits(word, PITCH_SHIFT, PITCH_BITS, units),
            None => word,
        }
    });
    patch_dword(&mut header, FETCH_1_DWORD, |word| {
        let word = set_bits(word, DATA_FORMAT_SHIFT, DATA_FORMAT_BITS, case.data_format);
        set_bits(word, ENDIAN_SHIFT, ENDIAN_BITS, case.endian)
    });
    patch_dword(&mut header, FETCH_3_DWORD, |word| {
        let word = set_bits(word, NUM_FORMAT_SHIFT, NUM_FORMAT_BITS, case.num_format);
        case.swizzle
            .iter()
            .zip(SWIZZLE_SHIFTS)
            .fold(word, |word, (channel, shift)| {
                set_bits(word, shift, SWIZZLE_BITS, *channel)
            })
    });
    for (dword, mask) in &case.flips {
        patch_dword(&mut header, *dword, |word| word ^ mask);
    }
    header
}
