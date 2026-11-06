//! Crafted texture headers for the golden corpus.
//!
//! Aurora writes linear DXT1 and DXT5 textures that AuroraAsset.dll never produces itself, so
//! their header signatures are found by decoding crafted headers with the DLL and recording
//! what it accepts. The video bytes are random: any bytes are valid BC blocks.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::golden::{
    Paths, bgra_to_rgba, os, run_harness, run_harness_allowing_refusal, sha256_hex,
};
use crate::synthetic::Prng;

pub const CRAFTED_DIRECTORY: &str = "crafted";

const HEADER_BYTE_LENGTH: usize = 52;
const DWORD_BYTES: usize = 4;
const BYTES_PER_PIXEL: usize = 4;
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
const BLOCKS_PER_PITCH_UNIT: usize = 8;
const DXT1_BLOCK_BYTES: usize = 8;
const BC3_BLOCK_BYTES: usize = 16;
const BC3_COLOUR_HALF_OFFSET: usize = 8;
const COLOUR_ENDPOINT_BYTES: usize = 2;

const SMALL_SIZE: (u32, u32) = (32, 32);
const ODD_SIZE: (u32, u32) = (130, 70);
const ENDIANS: [u32; 4] = [0, 1, 2, 3];
const RGBA: (&str, [u32; 4]) = ("rgba", [0, 1, 2, 3]);
const BGRA: (&str, [u32; 4]) = ("bgra", [2, 1, 0, 3]);
const ABGR: (&str, [u32; 4]) = ("abgr", [3, 2, 1, 0]);
const DXT1: (&str, u32) = ("dxt1", GPU_FORMAT_DXT1);
const DXT2_3: (&str, u32) = ("dxt2_3", GPU_FORMAT_DXT2_3);
const DXT4_5: (&str, u32) = ("dxt4_5", GPU_FORMAT_DXT4_5);
const PITCH_UNITS_WIDER_THAN_NEEDED: u32 = 12;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Crafted {
    name: String,
    header: String,
    video: String,
    decoded_sha256: String,
    dll_accepted: bool,
}

impl Crafted {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn video_file(&self) -> &str {
        &self.video
    }

    pub fn decoded_file(&self) -> Option<String> {
        self.dll_accepted
            .then(|| format!("{CRAFTED_DIRECTORY}/{}.rgba", self.name))
    }
}

#[derive(Clone, Copy)]
enum VideoPattern {
    Random,
    /// Both colour endpoints of every block are equal, probing the "first greater than second" test.
    EqualColourEndpoints,
}

struct Case {
    name: String,
    data_format: u32,
    endian: u32,
    swizzle: [u32; 4],
    num_format: u32,
    sign: u32,
    tiled: bool,
    pitch_units: Option<u32>,
    width: u32,
    height: u32,
    pattern: VideoPattern,
}

impl Case {
    fn new(
        (format_name, data_format): (&str, u32),
        endian: u32,
        (swizzle_name, swizzle): (&str, [u32; 4]),
        (width, height): (u32, u32),
    ) -> Self {
        Self {
            name: format!("{format_name}_e{endian}_{swizzle_name}_{width}x{height}"),
            data_format,
            endian,
            swizzle,
            num_format: 0,
            sign: SIGN_ALL_UNSIGNED,
            tiled: false,
            pitch_units: None,
            width,
            height,
            pattern: VideoPattern::Random,
        }
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

    fn with_equal_colour_endpoints(mut self) -> Self {
        self.pattern = VideoPattern::EqualColourEndpoints;
        self.labelled("equal_endpoints")
    }

    fn block_bytes(&self) -> usize {
        if self.data_format == GPU_FORMAT_DXT1 {
            DXT1_BLOCK_BYTES
        } else {
            BC3_BLOCK_BYTES
        }
    }

    fn row_pitch(&self) -> usize {
        match self.pitch_units {
            Some(units) => units as usize * BLOCKS_PER_PITCH_UNIT * self.block_bytes(),
            None => {
                let blocks_wide = (self.width as usize).div_ceil(BLOCK_SIDE_TEXELS);
                align(blocks_wide, BLOCKS_PER_ROW_ALIGNMENT) * self.block_bytes()
            }
        }
    }

    /// The zero-padded surface size the DLL needs for this header.
    /// Cases with the same bytes share one video file.
    fn video_file_name(&self) -> String {
        let pattern = match self.pattern {
            VideoPattern::Random => "random",
            VideoPattern::EqualColourEndpoints => "equal_endpoints",
        };
        let codec = match self.block_bytes() {
            DXT1_BLOCK_BYTES => "bc1",
            _ => "bc3",
        };
        let pitch = match self.pitch_units {
            Some(units) => format!("_pitch{units}"),
            None => String::new(),
        };
        format!(
            "{pattern}_{codec}_{}x{}{pitch}.vid",
            self.width, self.height
        )
    }

    fn video_len(&self) -> usize {
        let blocks_high = (self.height as usize).div_ceil(BLOCK_SIDE_TEXELS);
        self.row_pitch() * align(blocks_high, BLOCK_ROWS_ALIGNMENT)
    }

    /// Cases of one block size and width share their random bytes, so two cases whose headers
    /// differ in a probed field decode identically exactly when that field does not matter.
    fn video_seed(&self) -> u64 {
        PRNG_SEED_BASE + self.block_bytes() as u64 * VIDEO_SEED_BLOCK_STRIDE + u64::from(self.width)
    }

    fn video(&self) -> Vec<u8> {
        let mut prng = Prng::with_seed(self.video_seed());
        let mut video: Vec<u8> = (0..self.video_len()).map(|_| prng.next_byte()).collect();
        match self.pattern {
            VideoPattern::Random => {}
            VideoPattern::EqualColourEndpoints => self.equalise_colour_endpoints(&mut video),
        }
        video
    }

    fn equalise_colour_endpoints(&self, video: &mut [u8]) {
        let colour_offset = match self.block_bytes() {
            BC3_BLOCK_BYTES => BC3_COLOUR_HALF_OFFSET,
            _ => 0,
        };
        for block in video.chunks_exact_mut(self.block_bytes()) {
            let first = colour_offset..colour_offset + COLOUR_ENDPOINT_BYTES;
            let second_start = first.end;
            block.copy_within(first, second_start);
        }
    }
}

fn cases() -> Vec<Case> {
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
        cases.push(Case::new(format, 1, ABGR, SMALL_SIZE));
        cases.push(
            Case::new(format, 1, RGBA, ODD_SIZE).with_pitch_units(PITCH_UNITS_WIDER_THAN_NEEDED),
        );
        cases.push(Case::new(format, 1, RGBA, SMALL_SIZE).with_equal_colour_endpoints());
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

/// The header the DLL writes for a DXT texture of this size, used as the base to patch.
fn dll_dxt_header(
    paths: &Paths,
    work_dir: &Path,
    (width, height): (u32, u32),
) -> Result<[u8; HEADER_BYTE_LENGTH]> {
    let pixels = work_dir.join("base.argb");
    let header_path = work_dir.join("base.header");
    let video_path = work_dir.join("base.video");
    std::fs::write(
        &pixels,
        vec![0u8; width as usize * height as usize * BYTES_PER_PIXEL],
    )?;
    run_harness(
        paths,
        &[
            os("encode"),
            os("--dll"),
            paths.dll().into(),
            os("--input"),
            pixels.into(),
            os("--width"),
            width.to_string().into(),
            os("--height"),
            height.to_string().into(),
            os("--compression"),
            os("dxt"),
            os("--header-out"),
            header_path.clone().into(),
            os("--video-out"),
            video_path.into(),
        ],
    )?;
    let bytes = std::fs::read(&header_path)?;
    let length = bytes.len();
    bytes.try_into().map_err(|_| {
        anyhow::anyhow!("base header is {length} bytes, expected {HEADER_BYTE_LENGTH}")
    })
}

fn crafted_header(base: [u8; HEADER_BYTE_LENGTH], case: &Case) -> [u8; HEADER_BYTE_LENGTH] {
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
    header
}

/// Decodes every crafted case with the DLL and writes the videos and decoded pixels.
pub fn write_crafted(paths: &Paths, work_dir: &Path, out_dir: &Path) -> Result<Vec<Crafted>> {
    let crafted_dir = out_dir.join(CRAFTED_DIRECTORY);
    std::fs::create_dir_all(&crafted_dir)?;
    let small_base = dll_dxt_header(paths, work_dir, SMALL_SIZE)?;
    let odd_base = dll_dxt_header(paths, work_dir, ODD_SIZE)?;

    let mut crafted = Vec::new();
    for case in &cases() {
        let base = if (case.width, case.height) == SMALL_SIZE {
            small_base
        } else {
            odd_base
        };
        let header = crafted_header(base, case);
        let video = case.video();
        let video_file = format!("{CRAFTED_DIRECTORY}/{}", case.video_file_name());
        std::fs::write(out_dir.join(&video_file), &video)?;

        let header_path = work_dir.join("crafted.header");
        let video_path = work_dir.join("crafted.video");
        let decoded_path = work_dir.join("crafted.bgra");
        std::fs::write(&header_path, header)?;
        std::fs::write(&video_path, &video)?;
        if decoded_path.exists() {
            std::fs::remove_file(&decoded_path)?;
        }
        let report = run_harness_allowing_refusal(
            paths,
            &[
                os("decode"),
                os("--dll"),
                paths.dll().into(),
                os("--header"),
                header_path.into(),
                os("--video"),
                video_path.into(),
                os("--out"),
                decoded_path.clone().into(),
            ],
        )
        .with_context(|| format!("crafted case {}", case.name))?;

        let decoded_sha256 = match report {
            Some(report) => {
                ensure!(
                    report.trim() == format!("{} {}", case.width, case.height),
                    "{}: decoded size {:?} differs from {}x{}",
                    case.name,
                    report.trim(),
                    case.width,
                    case.height
                );
                let rgba = bgra_to_rgba(&std::fs::read(&decoded_path)?);
                std::fs::write(crafted_dir.join(format!("{}.rgba", case.name)), &rgba)?;
                sha256_hex(&rgba)
            }
            None => String::new(),
        };
        crafted.push(Crafted {
            name: case.name.clone(),
            header: hex::encode(header),
            video: video_file,
            dll_accepted: !decoded_sha256.is_empty(),
            decoded_sha256,
        });
    }
    Ok(crafted)
}
