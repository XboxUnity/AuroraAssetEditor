//! Encodes images with `aurora-asset`, decodes them with the original DLL and with `aurora-asset`,
//! and requires identical pixels.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use aurora_asset::{Compression, EncodedTexture, RgbaImage};
use aurora_image::{ImageTarget, decode_image, encoder_pixels, fit_to_target};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};

use crate::golden::{self, Paths, bgra_to_rgba, os};
use crate::synthetic;

const CROSS_DIRECTORY: &str = "cross";
const BYTES_PER_PIXEL: usize = 4;
const INPUT_EXTENSIONS: [&str; 1] = ["png"];
const CMYK_SAMPLE: &str = "jpeg_cmyk_900x600.jpg";
const CMYK_JPEG_QUALITY: u8 = 90;

/// A sample written by `cross-samples`: the synthetic image it is resized from, its file name, its
/// size, and the target the app resizes it to on import.
struct CrossSample {
    source: &'static str,
    file: &'static str,
    width: u32,
    height: u32,
    target: ImageTarget,
}

const CROSS_SAMPLES: [CrossSample; 9] = [
    CrossSample {
        source: "alphagrad",
        file: "webp_lossless_256.webp",
        width: 256,
        height: 256,
        target: ImageTarget::Icon,
    },
    CrossSample {
        source: "sines",
        file: "jpeg_900x600.jpg",
        width: 900,
        height: 600,
        target: ImageTarget::Cover,
    },
    CrossSample {
        source: "grad",
        file: "gif_64x64.gif",
        width: 64,
        height: 64,
        target: ImageTarget::Icon,
    },
    CrossSample {
        source: "alphanoise",
        file: "bmp_32_420x96.bmp",
        width: 420,
        height: 96,
        target: ImageTarget::Banner,
    },
    CrossSample {
        source: "alphagrad",
        file: "tiff_rgba_300x200.tif",
        width: 300,
        height: 200,
        target: ImageTarget::Cover,
    },
    CrossSample {
        source: "alphanoise",
        file: "tga_rgba_64x64.tga",
        width: 64,
        height: 64,
        target: ImageTarget::Icon,
    },
    CrossSample {
        source: "noise",
        file: "ico_48x48.ico",
        width: 48,
        height: 48,
        target: ImageTarget::Icon,
    },
    CrossSample {
        source: "sines",
        file: "qoi_rgba_640x360.qoi",
        width: 640,
        height: 360,
        target: ImageTarget::Background,
    },
    CrossSample {
        source: "sines",
        file: CMYK_SAMPLE,
        width: 900,
        height: 600,
        target: ImageTarget::Cover,
    },
];
const COMPRESSIONS: [(&str, Compression); 2] =
    [("DXT", Compression::Dxt), ("ARGB", Compression::None)];

struct Mismatch {
    differing_pixels: usize,
    first_differing_pixel: (u32, u32),
}

fn cross_directory(paths: &Paths) -> PathBuf {
    paths.corpus().join(CROSS_DIRECTORY)
}

fn synthetic_image(name: &str) -> Result<RgbaImage> {
    synthetic::all_synthetic_images()
        .into_iter()
        .find_map(|(candidate, image)| (candidate == name).then_some(image))
        .with_context(|| format!("no synthetic image named {name}"))
}

/// Writes the cross samples, resized from synthetic corpus images.
pub fn write_samples() -> Result<()> {
    let paths = Paths::new()?;
    let directory = cross_directory(&paths);
    std::fs::create_dir_all(&directory)?;
    for sample in &CROSS_SAMPLES {
        let resized = image::imageops::resize(
            &synthetic_image(sample.source)?,
            sample.width,
            sample.height,
            FilterType::Lanczos3,
        );
        let path = directory.join(sample.file);
        write_sample(&resized, &path)
            .with_context(|| format!("cannot write {}", path.display()))?;
        println!("{}: {}x{}", sample.file, sample.width, sample.height);
    }
    Ok(())
}

fn write_sample(image: &RgbaImage, path: &Path) -> Result<()> {
    if path.file_name().is_some_and(|name| name == CMYK_SAMPLE) {
        return write_cmyk_jpeg(image, path);
    }
    let format = ImageFormat::from_path(path)?;
    match format {
        ImageFormat::Jpeg => DynamicImage::ImageRgba8(image.clone())
            .to_rgb8()
            .save_with_format(path, format)?,
        _ => image.save_with_format(path, format)?,
    }
    Ok(())
}

/// The `image` crate cannot write CMYK, so the RGB pixels are converted by hand.
fn write_cmyk_jpeg(image: &RgbaImage, path: &Path) -> Result<()> {
    let cmyk: Vec<u8> = image
        .pixels()
        .flat_map(|pixel| [255 - pixel[0], 255 - pixel[1], 255 - pixel[2], 0])
        .collect();
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, CMYK_JPEG_QUALITY).encode(
        &cmyk,
        u16::try_from(image.width())?,
        u16::try_from(image.height())?,
        jpeg_encoder::ColorType::Cmyk,
    )?;
    std::fs::write(path, jpeg)?;
    Ok(())
}

fn files_with_extensions(directory: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("cannot read {}", directory.display()))?
    {
        let path = entry?.path();
        let matches_extension = path.extension().is_some_and(|extension| {
            extensions
                .iter()
                .any(|wanted| extension.eq_ignore_ascii_case(wanted))
        });
        if path.is_file() && matches_extension {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The corpus PNGs plus exactly the files of `CROSS_SAMPLES`; a missing sample is an error.
fn default_inputs(inputs_directory: &Path, cross_directory: &Path) -> Result<Vec<PathBuf>> {
    let mut inputs = files_with_extensions(inputs_directory, &INPUT_EXTENSIONS)?;
    for sample in &CROSS_SAMPLES {
        let path = cross_directory.join(sample.file);
        ensure!(
            path.is_file(),
            "cross sample {} is missing from {}; run `cargo xtask cross-samples`",
            sample.file,
            cross_directory.display()
        );
        inputs.push(path);
    }
    Ok(inputs)
}

/// The target the app resizes this file to on import; corpus images keep their own size.
fn target_for(file_name: &str) -> Option<ImageTarget> {
    CROSS_SAMPLES
        .iter()
        .find(|sample| sample.file == file_name)
        .map(|sample| sample.target)
}

/// The pixels the app hands the encoder when it imports `bytes`.
fn import_for_asset(
    bytes: &[u8],
    file_name: &str,
    target: Option<ImageTarget>,
) -> Result<RgbaImage> {
    let mut image = decode_image(bytes, Some(file_name))?;
    if let Some(target) = target {
        image = fit_to_target(image, target, true);
    }
    Ok(encoder_pixels(&image))
}

fn dll_decode(
    paths: &Paths,
    work_dir: &Path,
    texture: &EncodedTexture,
    expected_size: (u32, u32),
) -> Result<Vec<u8>> {
    let header_path = work_dir.join("cross.header");
    let video_path = work_dir.join("cross.video");
    let decoded_path = work_dir.join("cross.bgra");
    if decoded_path.exists() {
        std::fs::remove_file(&decoded_path)?;
    }
    std::fs::write(&header_path, texture.header.to_bytes())?;
    std::fs::write(&video_path, &texture.data)?;
    let report = golden::run_harness(
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
    )?;
    let (width, height) = expected_size;
    ensure!(
        report.trim() == format!("{width} {height}"),
        "the DLL decoded size {:?}, expected {width}x{height}",
        report.trim()
    );
    Ok(bgra_to_rgba(&std::fs::read(decoded_path)?))
}

fn compare_pixels(width: u32, ours: &[u8], dll: &[u8]) -> Option<Mismatch> {
    let differing: Vec<usize> = ours
        .as_chunks::<BYTES_PER_PIXEL>()
        .0
        .iter()
        .zip(dll.as_chunks::<BYTES_PER_PIXEL>().0)
        .enumerate()
        .filter_map(|(index, (ours, dll))| (ours != dll).then_some(index))
        .collect();
    let first = *differing.first()?;
    Some(Mismatch {
        differing_pixels: differing.len(),
        first_differing_pixel: ((first as u32) % width, (first as u32) / width),
    })
}

fn check_image(
    paths: &Paths,
    work_dir: &Path,
    name: &str,
    image: &RgbaImage,
) -> Result<Vec<String>> {
    let mut mismatches = Vec::new();
    for (label, compression) in COMPRESSIONS {
        let encoded = aurora_asset::encode(image, compression)
            .with_context(|| format!("{name}: {label} encode"))?;
        let ours =
            aurora_asset::decode(&encoded).with_context(|| format!("{name}: {label} decode"))?;
        let dll = dll_decode(paths, work_dir, &encoded, image.dimensions())
            .with_context(|| format!("{name}: {label} DLL decode"))?;
        if ours.as_raw().len() != dll.len() {
            bail!(
                "{name}: {label} decoded sizes differ ({} and {} bytes)",
                ours.as_raw().len(),
                dll.len()
            );
        }
        if let Some(mismatch) = compare_pixels(image.width(), ours.as_raw(), &dll) {
            mismatches.push(format!(
                "{name} {label}: {} of {} pixels differ, first at ({}, {})",
                mismatch.differing_pixels,
                image.width() * image.height(),
                mismatch.first_differing_pixel.0,
                mismatch.first_differing_pixel.1
            ));
        }
    }
    Ok(mismatches)
}

pub fn run(files: &[PathBuf]) -> Result<()> {
    golden::require_windows()?;
    let paths = Paths::new()?;
    golden::build_harness(&paths)?;
    let inputs = if files.is_empty() {
        default_inputs(&paths.inputs(), &cross_directory(&paths))?
    } else {
        files.to_vec()
    };
    ensure!(!inputs.is_empty(), "cross-decode found no images to check");
    let work_dir = tempfile::tempdir()?;
    let work_path = golden::absolute(work_dir.path())?;

    let mut mismatches = Vec::new();
    for input in &inputs {
        let name = input
            .file_name()
            .context("input has no file name")?
            .to_string_lossy()
            .into_owned();
        let bytes =
            std::fs::read(input).with_context(|| format!("cannot read {}", input.display()))?;
        let image = import_for_asset(&bytes, &name, target_for(&name))
            .with_context(|| format!("cannot import {}", input.display()))?;
        mismatches.extend(check_image(&paths, &work_path, &name, &image)?);
    }

    let encodings = inputs.len() * COMPRESSIONS.len();
    for mismatch in &mismatches {
        eprintln!("MISMATCH: {mismatch}");
    }
    if !mismatches.is_empty() {
        bail!(
            "cross-decode: {encodings} encodings checked, {} mismatches",
            mismatches.len()
        );
    }
    println!(
        "cross-decode: {} images, {encodings} encodings checked, 0 mismatches",
        inputs.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use aurora_image::IMPORT_EXTENSIONS;

    use super::*;

    const MARKER_PREFIX: u8 = 0xFF;
    const START_OF_FRAME_BASELINE: u8 = 0xC0;
    const START_OF_FRAME_PROGRESSIVE: u8 = 0xC2;
    const FRAME_COMPONENT_COUNT_OFFSET: usize = 9;

    #[test]
    fn every_import_format_has_a_cross_sample() {
        let imported: HashSet<ImageFormat> = IMPORT_EXTENSIONS
            .iter()
            .filter(|extension| **extension != "dds")
            .map(|extension| ImageFormat::from_extension(extension).expect("known extension"))
            .collect();
        let mut covered: HashSet<ImageFormat> = CROSS_SAMPLES
            .iter()
            .map(|sample| ImageFormat::from_path(sample.file).expect("image extension"))
            .collect();
        covered.insert(ImageFormat::Png);
        assert_eq!(imported, covered);
    }

    #[test]
    fn cmyk_sample_has_four_components() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives directly under the workspace root")
            .join("fixtures/codec/cross")
            .join(CMYK_SAMPLE);
        let jpeg = std::fs::read(&path).expect("the CMYK sample is committed");
        let frame_header = jpeg
            .windows(2)
            .position(|marker| {
                marker[0] == MARKER_PREFIX
                    && (START_OF_FRAME_BASELINE..=START_OF_FRAME_PROGRESSIVE).contains(&marker[1])
            })
            .expect("the sample has a start-of-frame marker");
        assert_eq!(jpeg[frame_header + FRAME_COMPONENT_COUNT_OFFSET], 4);
    }

    #[test]
    fn a_missing_cross_sample_is_an_error() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = CROSS_SAMPLES[0].file;
        for sample in &CROSS_SAMPLES[1..] {
            std::fs::write(directory.path().join(sample.file), b"x").expect("write sample");
        }
        let error =
            default_inputs(directory.path(), directory.path()).expect_err("a sample is missing");
        assert!(error.to_string().contains(missing), "{error}");
    }
}
