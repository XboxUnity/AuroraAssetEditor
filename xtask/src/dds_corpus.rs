//! DDS samples for the golden corpus: one file per format, size and mipmap choice, decoded by the
//! DLL's `ConvertDDSToImage`. The payloads are pseudo-random: any bytes are valid pixels or blocks.

use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::golden::{Paths, bgra_to_rgba, os, run_harness_allowing_refusal, sha256_hex};
use crate::synthetic::Prng;

pub const DDS_DIRECTORY: &str = "dds";

const BYTES_PER_PIXEL: usize = 4;
const DDS_MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: u32 = 124;
const PIXEL_FORMAT_SIZE: u32 = 32;
const HEADER_RESERVED_DWORDS: usize = 11;
const BLOCK_SIDE_TEXELS: u32 = 4;
const PRNG_SEED_BASE: u64 = 0xDD5_0000;

const FLAG_CAPS: u32 = 0x1;
const FLAG_HEIGHT: u32 = 0x2;
const FLAG_WIDTH: u32 = 0x4;
const FLAG_PITCH: u32 = 0x8;
const FLAG_PIXEL_FORMAT: u32 = 0x1000;
const FLAG_MIPMAP_COUNT: u32 = 0x2_0000;
const FLAG_LINEAR_SIZE: u32 = 0x8_0000;

const PIXEL_FLAG_ALPHA_PIXELS: u32 = 0x1;
const PIXEL_FLAG_FOURCC: u32 = 0x4;
const PIXEL_FLAG_RGB: u32 = 0x40;
const PIXEL_FLAG_LUMINANCE: u32 = 0x2_0000;

const CAPS_TEXTURE: u32 = 0x1000;
const CAPS_COMPLEX: u32 = 0x8;
const CAPS_MIPMAP: u32 = 0x40_0000;

/// The second size is not a multiple of four, so the compressed formats have partial edge blocks.
const DXT3_FOURCC: [u8; 4] = *b"DXT3";

const SIZES: [(u32, u32); 2] = [(16, 16), (18, 10)];

#[derive(Clone, Copy)]
enum Layout {
    Blocks {
        fourcc: [u8; 4],
        block_bytes: u32,
    },
    Pixels {
        bits: u32,
        pixel_flags: u32,
        masks: [u32; 4],
    },
}

const RGB_888_MASKS: [u32; 4] = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0];

const FORMATS: [(&str, Layout); 10] = [
    (
        "dxt1",
        Layout::Blocks {
            fourcc: *b"DXT1",
            block_bytes: 8,
        },
    ),
    (
        "dxt3",
        Layout::Blocks {
            fourcc: *b"DXT3",
            block_bytes: 16,
        },
    ),
    (
        "dxt5",
        Layout::Blocks {
            fourcc: *b"DXT5",
            block_bytes: 16,
        },
    ),
    (
        "a8r8g8b8",
        Layout::Pixels {
            bits: 32,
            pixel_flags: PIXEL_FLAG_RGB | PIXEL_FLAG_ALPHA_PIXELS,
            masks: [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000],
        },
    ),
    (
        "x8r8g8b8",
        Layout::Pixels {
            bits: 32,
            pixel_flags: PIXEL_FLAG_RGB,
            masks: RGB_888_MASKS,
        },
    ),
    (
        "r8g8b8",
        Layout::Pixels {
            bits: 24,
            pixel_flags: PIXEL_FLAG_RGB,
            masks: RGB_888_MASKS,
        },
    ),
    (
        "r5g6b5",
        Layout::Pixels {
            bits: 16,
            pixel_flags: PIXEL_FLAG_RGB,
            masks: [0xF800, 0x07E0, 0x001F, 0],
        },
    ),
    (
        "a1r5g5b5",
        Layout::Pixels {
            bits: 16,
            pixel_flags: PIXEL_FLAG_RGB | PIXEL_FLAG_ALPHA_PIXELS,
            masks: [0x7C00, 0x03E0, 0x001F, 0x8000],
        },
    ),
    (
        "a4r4g4b4",
        Layout::Pixels {
            bits: 16,
            pixel_flags: PIXEL_FLAG_RGB | PIXEL_FLAG_ALPHA_PIXELS,
            masks: [0x0F00, 0x00F0, 0x000F, 0xF000],
        },
    ),
    (
        "l8",
        Layout::Pixels {
            bits: 8,
            pixel_flags: PIXEL_FLAG_LUMINANCE,
            masks: [0xFF, 0, 0, 0],
        },
    ),
];

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct DdsSample {
    name: String,
    file: String,
    decoded_sha256: String,
    dll_accepted: bool,
    /// The DLL's output past the pixel data is uninitialised memory, so no hash is recorded.
    dll_output_unstable: bool,
    /// The DLL writes the partial block column of a width that is not a multiple of 4 into the
    /// next row; the port crops instead, so only acceptance is recorded.
    dll_layout_scrambled: bool,
    /// Length the DLL reports for the image; only this prefix is converted to BGRA by the DLL.
    reported_len: usize,
    /// Hash of the RGBA conversion of the DLL's first `reported_len` bytes, verbatim.
    verbatim_sha256: String,
}

impl DdsSample {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file(&self) -> &str {
        &self.file
    }
}

fn level_size(top: u32, level: u32) -> u32 {
    (top >> level).max(1)
}

fn level_count(width: u32, height: u32) -> u32 {
    width.max(height).ilog2() + 1
}

fn level_byte_len(layout: Layout, width: u32, height: u32) -> usize {
    match layout {
        Layout::Blocks { block_bytes, .. } => {
            let blocks_across = width.div_ceil(BLOCK_SIDE_TEXELS);
            let blocks_down = height.div_ceil(BLOCK_SIDE_TEXELS);
            (blocks_across * blocks_down * block_bytes) as usize
        }
        Layout::Pixels { bits, .. } => (width * height * bits / 8) as usize,
    }
}

fn header_bytes(layout: Layout, width: u32, height: u32, mip_levels: u32) -> Vec<u8> {
    let mut flags = FLAG_CAPS | FLAG_HEIGHT | FLAG_WIDTH | FLAG_PIXEL_FORMAT;
    let mut caps = CAPS_TEXTURE;
    if mip_levels > 1 {
        flags |= FLAG_MIPMAP_COUNT;
        caps |= CAPS_COMPLEX | CAPS_MIPMAP;
    }
    let (pitch_or_linear_size, pixel_flags, fourcc, bits, masks) = match layout {
        Layout::Blocks { fourcc, .. } => {
            flags |= FLAG_LINEAR_SIZE;
            let linear_size = level_byte_len(layout, width, height) as u32;
            (linear_size, PIXEL_FLAG_FOURCC, fourcc, 0, [0; 4])
        }
        Layout::Pixels {
            bits,
            pixel_flags,
            masks,
        } => {
            flags |= FLAG_PITCH;
            (width * bits / 8, pixel_flags, [0; 4], bits, masks)
        }
    };

    let mut words = vec![
        HEADER_SIZE,
        flags,
        height,
        width,
        pitch_or_linear_size,
        0,
        mip_levels,
    ];
    words.extend([0; HEADER_RESERVED_DWORDS]);
    words.extend([
        PIXEL_FORMAT_SIZE,
        pixel_flags,
        u32::from_le_bytes(fourcc),
        bits,
    ]);
    words.extend(masks);
    words.extend([caps, 0, 0, 0, 0]);

    let mut bytes = DDS_MAGIC.to_vec();
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
    bytes
}

fn random_bytes(prng: &mut Prng, len: usize) -> Vec<u8> {
    (0..len).map(|_| prng.next_byte()).collect()
}

fn sample_bytes(layout: Layout, width: u32, height: u32, with_mips: bool, seed: u64) -> Vec<u8> {
    let mip_levels = if with_mips {
        level_count(width, height)
    } else {
        1
    };
    let mut prng = Prng::with_seed(seed);
    let mut bytes = header_bytes(layout, width, height, mip_levels);
    for level in 0..mip_levels {
        let len = level_byte_len(layout, level_size(width, level), level_size(height, level));
        bytes.extend(random_bytes(&mut prng, len));
    }
    bytes
}

fn dll_layout_scrambled(layout: Layout, width: u32) -> bool {
    matches!(layout, Layout::Blocks { .. }) && !width.is_multiple_of(BLOCK_SIDE_TEXELS)
}

/// How the DLL treats a format, as found by probing it. Every decoded buffer is in ARGB byte
/// order except its first reported-length bytes, which the DLL swaps to BGRA.
#[derive(Clone, Copy, PartialEq)]
enum DllBehavior {
    Decoded,
    /// Decoded, but the DLL keeps the unused byte as alpha; the port makes it opaque.
    DecodedWithUnusedAlpha,
    /// Raw copy followed by uninitialised memory; DXT3 is never decoded.
    Unstable,
}

fn dll_behavior(layout: Layout) -> DllBehavior {
    match layout {
        Layout::Blocks {
            fourcc: DXT3_FOURCC,
            ..
        } => DllBehavior::Unstable,
        Layout::Blocks { .. } => DllBehavior::Decoded,
        Layout::Pixels {
            bits: 32, masks, ..
        } if masks[3] != 0 => DllBehavior::Decoded,
        Layout::Pixels { bits: 32, .. } => DllBehavior::DecodedWithUnusedAlpha,
        Layout::Pixels { .. } => DllBehavior::Unstable,
    }
}

struct DllDecode {
    buffer: Vec<u8>,
    reported_len: usize,
}

/// The DLL's full output buffer, or `None` when it refuses the file.
fn dll_decode(
    paths: &Paths,
    work_dir: &Path,
    name: &str,
    dds_path: &Path,
) -> Result<Option<DllDecode>> {
    let decoded_path = work_dir.join("dds.bgra");
    if decoded_path.exists() {
        std::fs::remove_file(&decoded_path)?;
    }
    let report = run_harness_allowing_refusal(
        paths,
        &[
            os("decode-dds"),
            os("--dll"),
            paths.dll().into(),
            os("--input"),
            dds_path.into(),
            os("--out"),
            decoded_path.clone().into(),
        ],
    )
    .with_context(|| format!("DDS sample {name}"))?;
    let Some(report) = report else {
        return Ok(None);
    };
    let numbers = report
        .split_whitespace()
        .map(str::parse::<usize>)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("{name}: unreadable decode report {:?}", report.trim()))?;
    let [width, height, reported_len] = numbers[..] else {
        anyhow::bail!("{name}: decode report {:?} is not `w h len`", report.trim());
    };
    let buffer = std::fs::read(&decoded_path)?;
    ensure!(
        buffer.len() == width * height * BYTES_PER_PIXEL,
        "{name}: decode report {:?} does not match {} output bytes",
        report.trim(),
        buffer.len()
    );
    Ok(Some(DllDecode {
        buffer,
        reported_len,
    }))
}

/// Completes the DLL's ARGB to BGRA swap past the reported length, and makes the unused byte of
/// X8R8G8B8 opaque.
fn intended_bgra(behavior: DllBehavior, decode: &DllDecode) -> Vec<u8> {
    let mut bgra = decode.buffer.clone();
    // The DLL swaps whole pixels only: a reported length below 4 leaves every pixel in ARGB.
    let first_unswapped = decode.reported_len / BYTES_PER_PIXEL * BYTES_PER_PIXEL;
    for pixel in bgra[first_unswapped..].as_chunks_mut::<BYTES_PER_PIXEL>().0 {
        pixel.reverse();
    }
    if behavior == DllBehavior::DecodedWithUnusedAlpha {
        for pixel in bgra.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
            pixel[3] = u8::MAX;
        }
    }
    bgra
}

/// Writes every DDS sample into `out_dir` and decodes it with the DLL.
pub fn write_dds_samples(paths: &Paths, work_dir: &Path, out_dir: &Path) -> Result<Vec<DdsSample>> {
    let dds_dir = out_dir.join(DDS_DIRECTORY);
    std::fs::create_dir_all(&dds_dir)?;

    let mut samples = Vec::new();
    for (format_name, layout) in FORMATS {
        let behavior = dll_behavior(layout);
        for (width, height) in SIZES {
            for with_mips in [false, true] {
                let mips = if with_mips { "_mips" } else { "" };
                let name = format!("{format_name}{mips}_{width}x{height}");
                let seed = PRNG_SEED_BASE + samples.len() as u64;
                let bytes = sample_bytes(layout, width, height, with_mips, seed);
                let dds_path = dds_dir.join(format!("{name}.dds"));
                std::fs::write(&dds_path, &bytes)?;

                let mut sample = DdsSample {
                    file: format!("{DDS_DIRECTORY}/{name}.dds"),
                    name,
                    decoded_sha256: String::new(),
                    dll_accepted: false,
                    dll_output_unstable: false,
                    dll_layout_scrambled: false,
                    reported_len: 0,
                    verbatim_sha256: String::new(),
                };
                if let Some(decode) = dll_decode(paths, work_dir, &sample.name, &dds_path)? {
                    sample.dll_accepted = true;
                    sample.reported_len = decode.reported_len;
                    if behavior == DllBehavior::Unstable {
                        sample.dll_output_unstable = true;
                    } else if dll_layout_scrambled(layout, width) {
                        sample.dll_layout_scrambled = true;
                    } else {
                        let intended = intended_bgra(behavior, &decode);
                        sample.decoded_sha256 = sha256_hex(&bgra_to_rgba(&intended));
                        if behavior == DllBehavior::Decoded {
                            let verbatim = &decode.buffer[..decode.reported_len];
                            sample.verbatim_sha256 = sha256_hex(&bgra_to_rgba(verbatim));
                        }
                    }
                }
                samples.push(sample);
            }
        }
    }
    Ok(samples)
}
