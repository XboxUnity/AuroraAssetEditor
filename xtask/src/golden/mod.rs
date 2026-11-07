//! Generates and verifies the golden codec corpus produced by the original AuroraAsset.dll.

mod compression;
mod encoded;
mod harness;
mod manifest;
mod paths;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};
use sha2::{Digest, Sha256};

use crate::crafted::{self, CRAFTED_DIRECTORY, Crafted};
use crate::dds_sample::{self, DDS_DIRECTORY, DdsSample};
use crate::synthetic;
use compression::Compression;
use encoded::encode_and_decode;
use manifest::{Fixture, Manifest};

pub(crate) use harness::{
    build_harness, dll_decode, os, run_harness, run_harness_allowing_refusal,
};
pub(crate) use paths::{Paths, absolute};

const DXT_DIRECTORY: &str = "dxt";
const ARGB_DIRECTORY: &str = "argb";
const DECODED_DIRECTORY: &str = "decoded";
/// The corpus directories `golden generate` writes; `golden check` refuses any other file in them.
const GENERATED_DIRECTORIES: [&str; 5] = [
    DXT_DIRECTORY,
    ARGB_DIRECTORY,
    DECODED_DIRECTORY,
    CRAFTED_DIRECTORY,
    DDS_DIRECTORY,
];
const MASTER_PLACEHOLDER_DIRECTORY: &str = "AuroraAssetEditor/Resources/Placeholders";
/// ARGB videos of larger fixtures are checked by hash only, keeping the corpus under 10 MB.
const MAX_STORED_ARGB_PIXELS: u32 = 65_536;
const DXT_DECODED_STORED_NAMES: [&str; 2] = ["alphagrad", "alphanoise"];
const DXT_DECODED_STORED_PREFIX: &str = "odd_";
const BYTES_PER_PIXEL: usize = 4;

/// Placeholder name on `master`, fixture name, and the size the app resizes it to.
const PLACEHOLDERS: [(&str, &str, u32, u32); 5] = [
    ("cover", "ph_cover", 900, 600),
    ("background", "ph_background", 1280, 720),
    ("banner", "ph_banner", 420, 96),
    ("icon", "ph_icon", 64, 64),
    ("screenshot", "ph_screenshot", 1000, 562),
];

/// The DLL's DXT decode is stored for the fixtures where our decode is compared pixel by pixel:
/// the ones with known rounding ties and the odd-sized ones.
fn dxt_decoded_is_stored(name: &str) -> bool {
    DXT_DECODED_STORED_NAMES.contains(&name) || name.starts_with(DXT_DECODED_STORED_PREFIX)
}

pub(crate) fn require_windows() -> Result<()> {
    ensure!(
        cfg!(windows),
        "the golden corpus can only be generated or checked on Windows"
    );
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in bgra.as_chunks::<BYTES_PER_PIXEL>().0.iter() {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    rgba
}

fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    let mut argb = Vec::with_capacity(rgba.len());
    for pixel in rgba.as_chunks::<BYTES_PER_PIXEL>().0.iter() {
        argb.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
    }
    argb
}

fn sorted_input_pngs(inputs_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut pngs = Vec::new();
    for entry in std::fs::read_dir(inputs_dir)
        .with_context(|| format!("cannot read {}", inputs_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "png") {
            pngs.push(path);
        }
    }
    pngs.sort();
    Ok(pngs)
}

/// Runs the DLL over every committed input and writes the fixtures and manifest into `out_dir`.
fn write_goldens(paths: &Paths, inputs_dir: &Path, out_dir: &Path) -> Result<Manifest> {
    let dxt_dir = out_dir.join(DXT_DIRECTORY);
    let argb_dir = out_dir.join(ARGB_DIRECTORY);
    let decoded_dir = out_dir.join(DECODED_DIRECTORY);
    std::fs::create_dir_all(&dxt_dir)?;
    std::fs::create_dir_all(&argb_dir)?;
    std::fs::create_dir_all(&decoded_dir)?;
    let work_dir = tempfile::tempdir()?;
    let work_path = absolute(work_dir.path())?;

    let mut fixtures = Vec::new();
    for png_path in sorted_input_pngs(inputs_dir)? {
        let name = png_path
            .file_stem()
            .context("input has no file name")?
            .to_string_lossy()
            .into_owned();
        let image = image::open(&png_path)
            .with_context(|| format!("cannot read {}", png_path.display()))?
            .to_rgba8();
        let (width, height) = image.dimensions();

        let dxt = encode_and_decode(paths, &work_path, &image, Compression::Dxt)
            .with_context(|| format!("{name}: DXT"))?;
        let argb = encode_and_decode(paths, &work_path, &image, Compression::None)
            .with_context(|| format!("{name}: ARGB"))?;
        let argb_video_stored = width * height <= MAX_STORED_ARGB_PIXELS;

        let dxt_decoded_stored = dxt_decoded_is_stored(&name);
        std::fs::write(dxt_dir.join(format!("{name}.vid")), &dxt.video)?;
        if dxt_decoded_stored {
            std::fs::write(
                decoded_dir.join(format!("{name}.dxt.rgba")),
                &dxt.decoded_rgba,
            )?;
        }
        if argb_video_stored {
            std::fs::write(argb_dir.join(format!("{name}.vid")), &argb.video)?;
        }
        println!("{name}: {width}x{height}");
        fixtures.push(Fixture {
            name,
            width,
            height,
            dxt_header: hex::encode(dxt.header),
            dxt_video_sha256: sha256_hex(&dxt.video),
            dxt_decoded_sha256: sha256_hex(&dxt.decoded_rgba),
            argb_header: hex::encode(argb.header),
            argb_video_sha256: sha256_hex(&argb.video),
            argb_decoded_sha256: sha256_hex(&argb.decoded_rgba),
            argb_video_stored,
            dxt_decoded_stored,
            dxt_decodes_at_minimum_len: dxt.decodes_at_minimum_len,
            argb_decodes_at_minimum_len: argb.decodes_at_minimum_len,
        });
    }
    let crafted = crafted::write_crafted(paths, &work_path, out_dir)?;
    let dds = dds_sample::write_dds_samples(paths, &work_path, out_dir)?;
    let manifest = Manifest {
        fixture: fixtures,
        crafted,
        dds,
    };
    std::fs::write(out_dir.join("manifest.toml"), toml::to_string(&manifest)?)?;
    Ok(manifest)
}

fn master_placeholder_png(paths: &Paths, placeholder: &str) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(&paths.root)
        .arg("show")
        .arg(format!(
            "master:{MASTER_PLACEHOLDER_DIRECTORY}/{placeholder}.png"
        ))
        .output()
        .context("cannot start git")?;
    ensure!(
        output.status.success(),
        "git show for placeholder {placeholder} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

fn write_png(image: &RgbaImage, path: &Path) -> Result<()> {
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("cannot write {}", path.display()))
}

fn write_inputs(paths: &Paths) -> Result<()> {
    let inputs_dir = paths.inputs();
    std::fs::create_dir_all(&inputs_dir)?;
    for (placeholder, name, width, height) in PLACEHOLDERS {
        let source =
            image::load_from_memory(&master_placeholder_png(paths, placeholder)?)?.to_rgba8();
        let resized = image::imageops::resize(&source, width, height, FilterType::Lanczos3);
        write_png(&resized, &inputs_dir.join(format!("{name}.png")))?;
    }
    for (name, image) in synthetic::all_synthetic_images() {
        write_png(&image, &inputs_dir.join(format!("{name}.png")))?;
    }
    Ok(())
}

/// Writes the inputs only when missing (or on refresh), then regenerates the goldens from them.
pub fn generate(refresh_inputs: bool) -> Result<()> {
    require_windows()?;
    let paths = Paths::new()?;
    let inputs_missing = !paths.inputs().is_dir() || sorted_input_pngs(&paths.inputs())?.is_empty();
    if inputs_missing || refresh_inputs {
        write_inputs(&paths)?;
    }
    build_harness(&paths)?;
    let manifest = write_goldens(&paths, &paths.inputs(), &paths.corpus())?;
    println!(
        "{} fixtures, {} crafted cases and {} DDS samples written",
        manifest.fixture.len(),
        manifest.crafted.len(),
        manifest.dds.len()
    );
    Ok(())
}

fn compare_file(committed: &Path, regenerated: &Path, mismatches: &mut Vec<String>) -> Result<()> {
    match std::fs::read(committed) {
        Ok(bytes) if bytes == std::fs::read(regenerated)? => {}
        Ok(_) => mismatches.push(format!(
            "{} differs from the DLL output",
            committed.display()
        )),
        Err(error) => mismatches.push(format!("{} cannot be read: {error}", committed.display())),
    }
    Ok(())
}

fn compare_manifests(committed: &Manifest, regenerated: &Manifest, mismatches: &mut Vec<String>) {
    for fixture in &regenerated.fixture {
        let committed_entry = committed
            .fixture
            .iter()
            .find(|other| other.name == fixture.name);
        if committed_entry != Some(fixture) {
            mismatches.push(format!("manifest entry for {} differs", fixture.name));
        }
    }
    for fixture in &committed.fixture {
        if !regenerated
            .fixture
            .iter()
            .any(|other| other.name == fixture.name)
        {
            mismatches.push(format!("manifest entry for {} has no input", fixture.name));
        }
    }
    for case in &regenerated.crafted {
        let committed_entry = committed
            .crafted
            .iter()
            .find(|other| other.name() == case.name());
        if committed_entry != Some(case) {
            mismatches.push(format!(
                "manifest entry for crafted {} differs",
                case.name()
            ));
        }
    }
    for case in &committed.crafted {
        if !regenerated
            .crafted
            .iter()
            .any(|other| other.name() == case.name())
        {
            mismatches.push(format!(
                "manifest entry for crafted {} is no longer generated",
                case.name()
            ));
        }
    }
    for sample in &regenerated.dds {
        let committed_entry = committed
            .dds
            .iter()
            .find(|other| other.name() == sample.name());
        if committed_entry != Some(sample) {
            mismatches.push(format!("manifest entry for DDS {} differs", sample.name()));
        }
    }
    for sample in &committed.dds {
        if !regenerated
            .dds
            .iter()
            .any(|other| other.name() == sample.name())
        {
            mismatches.push(format!(
                "manifest entry for DDS {} is no longer generated",
                sample.name()
            ));
        }
    }
}

fn report_duplicates<'a>(
    kind: &str,
    names: impl Iterator<Item = &'a str>,
    mismatches: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name) {
            mismatches.push(format!("the manifest lists {kind} {name} more than once"));
        }
    }
}

/// Every file in the generated directories of `corpus`, relative to it.
fn generated_directory_files(corpus: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    for directory in GENERATED_DIRECTORIES {
        let path = corpus.join(directory);
        if !path.is_dir() {
            continue;
        }
        for entry in
            std::fs::read_dir(&path).with_context(|| format!("cannot read {}", path.display()))?
        {
            files.insert(Path::new(directory).join(entry?.file_name()));
        }
    }
    Ok(files)
}

/// Committed files that `golden generate` no longer writes, so that no manifest entry checks.
fn report_unreferenced_files(
    committed_corpus: &Path,
    regenerated_corpus: &Path,
    mismatches: &mut Vec<String>,
) -> Result<()> {
    let generated = generated_directory_files(regenerated_corpus)?;
    for file in generated_directory_files(committed_corpus)?.difference(&generated) {
        mismatches.push(format!(
            "{} is not written by golden generate",
            committed_corpus.join(file).display()
        ));
    }
    Ok(())
}

/// Regenerates into a temp directory from the committed inputs and compares with the committed
/// corpus.
pub fn check() -> Result<()> {
    require_windows()?;
    let paths = Paths::new()?;
    build_harness(&paths)?;
    let temp = tempfile::tempdir()?;
    let regenerated_dir = absolute(temp.path())?;
    let regenerated = write_goldens(&paths, &paths.inputs(), &regenerated_dir)?;

    let committed_text = std::fs::read_to_string(paths.corpus().join("manifest.toml"))
        .context("cannot read the committed manifest.toml")?;
    let committed: Manifest =
        toml::from_str(&committed_text).context("committed manifest.toml does not parse")?;

    let mut mismatches = Vec::new();
    report_duplicates(
        "fixture",
        committed
            .fixture
            .iter()
            .map(|fixture| fixture.name.as_str()),
        &mut mismatches,
    );
    report_duplicates(
        "crafted case",
        committed.crafted.iter().map(Crafted::name),
        &mut mismatches,
    );
    report_duplicates(
        "DDS sample",
        committed.dds.iter().map(DdsSample::name),
        &mut mismatches,
    );
    report_unreferenced_files(&paths.corpus(), &regenerated_dir, &mut mismatches)?;
    compare_manifests(&committed, &regenerated, &mut mismatches);
    for fixture in &regenerated.fixture {
        let relative_dxt = Path::new(DXT_DIRECTORY).join(format!("{}.vid", fixture.name));
        compare_file(
            &paths.corpus().join(&relative_dxt),
            &regenerated_dir.join(&relative_dxt),
            &mut mismatches,
        )?;
        if fixture.dxt_decoded_stored {
            let relative_decoded =
                Path::new(DECODED_DIRECTORY).join(format!("{}.dxt.rgba", fixture.name));
            compare_file(
                &paths.corpus().join(&relative_decoded),
                &regenerated_dir.join(&relative_decoded),
                &mut mismatches,
            )?;
        }
        if fixture.argb_video_stored {
            let relative_argb = Path::new(ARGB_DIRECTORY).join(format!("{}.vid", fixture.name));
            compare_file(
                &paths.corpus().join(&relative_argb),
                &regenerated_dir.join(&relative_argb),
                &mut mismatches,
            )?;
        }
    }
    for case in &regenerated.crafted {
        let mut relative_files = vec![case.video_file().to_string()];
        relative_files.extend(case.decoded_file());
        for relative in relative_files {
            compare_file(
                &paths.corpus().join(&relative),
                &regenerated_dir.join(&relative),
                &mut mismatches,
            )?;
        }
    }

    for sample in &regenerated.dds {
        compare_file(
            &paths.corpus().join(sample.file()),
            &regenerated_dir.join(sample.file()),
            &mut mismatches,
        )?;
    }

    if mismatches.is_empty() {
        println!(
            "golden corpus matches ({} fixtures, {} crafted cases, {} DDS samples)",
            regenerated.fixture.len(),
            regenerated.crafted.len(),
            regenerated.dds.len()
        );
        return Ok(());
    }
    for mismatch in &mismatches {
        eprintln!("MISMATCH: {mismatch}");
    }
    bail!("{} golden mismatches", mismatches.len())
}
