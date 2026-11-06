//! Generates and verifies the golden codec corpus produced by the original AuroraAsset.dll.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::synthetic;

const HARNESS_TARGET: &str = "i686-pc-windows-msvc";
const HARNESS_PACKAGE: &str = "golden-harness";
const MASTER_PLACEHOLDER_DIRECTORY: &str = "AuroraAssetEditor/Resources/Placeholders";
const MAX_STORED_ARGB_PIXELS: u32 = 65_536;
const BYTES_PER_PIXEL: usize = 4;
const HEADER_BYTE_LENGTH: usize = 52;
const HARNESS_REFUSED_EXIT_CODE: i32 = 4;
const VERBATIM_PATH_PREFIX: &str = r"\\?\";
const UNC_PATH_MARKER: &str = "UNC";

/// Placeholder name on `master`, fixture name, and the size the app resizes it to.
const PLACEHOLDERS: [(&str, &str, u32, u32); 5] = [
    ("cover", "ph_cover", 900, 600),
    ("background", "ph_background", 1280, 720),
    ("banner", "ph_banner", 420, 96),
    ("icon", "ph_icon", 64, 64),
    ("screenshot", "ph_screenshot", 1000, 562),
];

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Manifest {
    fixture: Vec<Fixture>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Fixture {
    name: String,
    width: u32,
    height: u32,
    dxt_header: String,
    dxt_video_sha256: String,
    dxt_decoded_sha256: String,
    argb_header: String,
    argb_video_sha256: String,
    argb_decoded_sha256: String,
    argb_video_stored: bool,
}

#[derive(Clone, Copy)]
enum Compression {
    Dxt,
    None,
}

impl Compression {
    fn harness_name(self) -> &'static str {
        match self {
            Compression::Dxt => "dxt",
            Compression::None => "none",
        }
    }
}

struct Encoded {
    header_hex: String,
    video: Vec<u8>,
    decoded_rgba_sha256: String,
}

struct Paths {
    root: PathBuf,
}

impl Paths {
    fn new() -> Result<Self> {
        let xtask_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = xtask_directory
            .parent()
            .context("xtask has no parent directory")?;
        Ok(Self {
            root: absolute(root)?,
        })
    }

    fn corpus(&self) -> PathBuf {
        self.root.join("fixtures").join("codec")
    }

    fn inputs(&self) -> PathBuf {
        self.corpus().join("inputs")
    }

    fn dll(&self) -> PathBuf {
        self.root
            .join("tools")
            .join("golden")
            .join("dll")
            .join("AuroraAsset.dll")
    }

    fn harness(&self) -> PathBuf {
        self.root
            .join("target")
            .join(HARNESS_TARGET)
            .join("release")
            .join("golden-harness.exe")
    }
}

/// Canonical path without the `\\?\` prefix, which the DLL loader's altered search path mishandles.
fn absolute(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    let text = canonical.to_string_lossy();
    match text.strip_prefix(VERBATIM_PATH_PREFIX) {
        Some(plain) if !plain.starts_with(UNC_PATH_MARKER) => Ok(PathBuf::from(plain)),
        _ => Ok(canonical),
    }
}

fn require_windows() -> Result<()> {
    ensure!(
        cfg!(windows),
        "the golden corpus can only be generated or checked on Windows"
    );
    Ok(())
}

fn cargo_command() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn build_harness(paths: &Paths) -> Result<()> {
    let status = cargo_command()
        .current_dir(&paths.root)
        .args([
            "build",
            "-p",
            HARNESS_PACKAGE,
            "--release",
            "--target",
            HARNESS_TARGET,
        ])
        .status()
        .context("cannot start cargo to build the harness")?;
    ensure!(
        status.success(),
        "building the golden harness failed: {status}"
    );
    Ok(())
}

fn run_harness(paths: &Paths, arguments: &[OsString]) -> Result<String> {
    let output = Command::new(paths.harness())
        .args(arguments)
        .output()
        .context("cannot start the golden harness")?;
    if !output.status.success() {
        let reason = match output.status.code() {
            Some(HARNESS_REFUSED_EXIT_CODE) => "the DLL refused the input".to_string(),
            _ => format!("exit status {}", output.status),
        };
        bail!(
            "golden harness failed ({reason}): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn os(text: &str) -> OsString {
    OsString::from(text)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
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

fn encode_and_decode(
    paths: &Paths,
    work_dir: &Path,
    image: &RgbaImage,
    compression: Compression,
) -> Result<Encoded> {
    let (width, height) = image.dimensions();
    let argb_path = work_dir.join("input.argb");
    let header_path = work_dir.join("out.header");
    let video_path = work_dir.join("out.video");
    let decoded_path = work_dir.join("out.bgra");
    std::fs::write(&argb_path, rgba_to_argb(image.as_raw()))?;

    run_harness(
        paths,
        &[
            os("encode"),
            os("--dll"),
            paths.dll().into(),
            os("--input"),
            argb_path.into(),
            os("--width"),
            width.to_string().into(),
            os("--height"),
            height.to_string().into(),
            os("--compression"),
            os(compression.harness_name()),
            os("--header-out"),
            header_path.clone().into(),
            os("--video-out"),
            video_path.clone().into(),
        ],
    )?;
    let decode_report = run_harness(
        paths,
        &[
            os("decode"),
            os("--dll"),
            paths.dll().into(),
            os("--header"),
            header_path.clone().into(),
            os("--video"),
            video_path.clone().into(),
            os("--out"),
            decoded_path.clone().into(),
        ],
    )?;
    ensure!(
        decode_report.trim() == format!("{width} {height}"),
        "decoded size {:?} differs from the input size {width}x{height}",
        decode_report.trim()
    );

    let header = std::fs::read(&header_path)?;
    ensure!(
        header.len() == HEADER_BYTE_LENGTH,
        "header is {} bytes, expected {HEADER_BYTE_LENGTH}",
        header.len()
    );
    let decoded_bgra = std::fs::read(&decoded_path)?;
    Ok(Encoded {
        header_hex: hex::encode(&header),
        video: std::fs::read(&video_path)?,
        decoded_rgba_sha256: sha256_hex(&bgra_to_rgba(&decoded_bgra)),
    })
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
    let dxt_dir = out_dir.join("dxt");
    let argb_dir = out_dir.join("argb");
    std::fs::create_dir_all(&dxt_dir)?;
    std::fs::create_dir_all(&argb_dir)?;
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

        std::fs::write(dxt_dir.join(format!("{name}.vid")), &dxt.video)?;
        if argb_video_stored {
            std::fs::write(argb_dir.join(format!("{name}.vid")), &argb.video)?;
        }
        println!("{name}: {width}x{height}");
        fixtures.push(Fixture {
            name,
            width,
            height,
            dxt_header: dxt.header_hex,
            dxt_video_sha256: sha256_hex(&dxt.video),
            dxt_decoded_sha256: dxt.decoded_rgba_sha256,
            argb_header: argb.header_hex,
            argb_video_sha256: sha256_hex(&argb.video),
            argb_decoded_sha256: argb.decoded_rgba_sha256,
            argb_video_stored,
        });
    }
    let manifest = Manifest { fixture: fixtures };
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
    println!("{} fixtures written", manifest.fixture.len());
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
}

/// Regenerates into a temp directory from the committed inputs and compares with the committed corpus.
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
    compare_manifests(&committed, &regenerated, &mut mismatches);
    for fixture in &regenerated.fixture {
        let relative_dxt = Path::new("dxt").join(format!("{}.vid", fixture.name));
        compare_file(
            &paths.corpus().join(&relative_dxt),
            &regenerated_dir.join(&relative_dxt),
            &mut mismatches,
        )?;
        if fixture.argb_video_stored {
            let relative_argb = Path::new("argb").join(format!("{}.vid", fixture.name));
            compare_file(
                &paths.corpus().join(&relative_argb),
                &regenerated_dir.join(&relative_argb),
                &mut mismatches,
            )?;
        }
    }

    if mismatches.is_empty() {
        println!(
            "golden corpus matches ({} fixtures)",
            regenerated.fixture.len()
        );
        return Ok(());
    }
    for mismatch in &mismatches {
        eprintln!("MISMATCH: {mismatch}");
    }
    bail!("{} golden mismatches", mismatches.len())
}
