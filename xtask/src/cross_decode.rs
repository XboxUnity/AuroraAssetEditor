//! Encodes images with `aurora-asset`, decodes them with the original DLL and with `aurora-asset`,
//! and requires identical pixels.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use aurora_asset::{Compression, EncodedTexture, RgbaImage};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};

use crate::golden::{self, Paths, bgra_to_rgba, os};
use crate::synthetic;

const CROSS_DIRECTORY: &str = "cross";
const BYTES_PER_PIXEL: usize = 4;
const INPUT_EXTENSIONS: [&str; 1] = ["png"];
const CROSS_EXTENSIONS: [&str; 4] = ["webp", "jpg", "jpeg", "gif"];

/// Synthetic source image, sample file name, and the size it is resized to.
const WEBP_SAMPLE: (&str, &str, u32, u32) = ("alphagrad", "webp_lossless_256.webp", 256, 256);
const JPEG_SAMPLE: (&str, &str, u32, u32) = ("sines", "jpeg_900x600.jpg", 900, 600);
const GIF_SAMPLE: (&str, &str, u32, u32) = ("grad", "gif_64x64.gif", 64, 64);
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

/// Writes the WebP, JPEG and GIF samples, resized from synthetic corpus images.
pub fn write_samples() -> Result<()> {
    let paths = Paths::new()?;
    let directory = cross_directory(&paths);
    std::fs::create_dir_all(&directory)?;
    for (source, file, width, height) in [WEBP_SAMPLE, JPEG_SAMPLE, GIF_SAMPLE] {
        let resized = image::imageops::resize(
            &synthetic_image(source)?,
            width,
            height,
            FilterType::Lanczos3,
        );
        let path = directory.join(file);
        let format = ImageFormat::from_path(&path)?;
        let written = match format {
            ImageFormat::Jpeg => DynamicImage::ImageRgba8(resized)
                .to_rgb8()
                .save_with_format(&path, format),
            _ => resized.save_with_format(&path, format),
        };
        written.with_context(|| format!("cannot write {}", path.display()))?;
        println!("{file}: {width}x{height}");
    }
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

fn default_inputs(paths: &Paths) -> Result<Vec<PathBuf>> {
    let mut inputs = files_with_extensions(&paths.inputs(), &INPUT_EXTENSIONS)?;
    inputs.extend(files_with_extensions(
        &cross_directory(paths),
        &CROSS_EXTENSIONS,
    )?);
    Ok(inputs)
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
        default_inputs(&paths)?
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
        let image = image::open(input)
            .with_context(|| format!("cannot read {}", input.display()))?
            .to_rgba8();
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
