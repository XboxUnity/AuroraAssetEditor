//! `cargo xtask gdiplus-report`: measures the Rust image pipeline against GDI+ (spec 6.2) and
//! writes `docs/image/gdiplus-closeness.md`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use aurora_image::{RESIZE_FILTER, Resampler, decode_image, encoder_pixels};
use image::RgbaImage;

use crate::closeness_inputs::{self, gate_directory};
use crate::golden::{Paths, absolute};

const REPORT_PATH: [&str; 3] = ["docs", "image", "gdiplus-closeness.md"];
const REFERENCE_DIRECTORY: &str = "gdiplus";
const REFERENCE_EXTENSION: &str = "argb";
const BYTES_PER_PIXEL: usize = 4;
const ALPHA: usize = 0;
const MAX_CHANNEL: f64 = 255.0;
/// An exact match has infinite PSNR; the mean counts it as this value.
const EXACT_PSNR_DB: f64 = 100.0;
const WINDOWS_ROOT_VARIABLE: &str = "SystemRoot";
/// The old app ran as a 32-bit process, so the reference runs in 32-bit Windows PowerShell.
const POWERSHELL_32_BIT: [&str; 4] = ["SysWOW64", "WindowsPowerShell", "v1.0", "powershell.exe"];
const REFERENCE_SCRIPT: [&str; 3] = ["tools", "gdiplus", "gdiplus-reference.ps1"];

/// How far one image is from its GDI+ reference.
#[derive(Clone, Copy)]
struct Closeness {
    differing_pixels: usize,
    max_difference: u8,
    psnr_db: f64,
}

impl Closeness {
    fn measure(ours: &[u8], gdiplus: &[u8]) -> Self {
        let mut squared_error = 0.0;
        let mut max_difference = 0;
        for (&ours, &gdiplus) in ours.iter().zip(gdiplus) {
            let difference = ours.abs_diff(gdiplus);
            max_difference = max_difference.max(difference);
            squared_error += f64::from(difference) * f64::from(difference);
        }
        let differing_pixels = ours
            .as_chunks::<BYTES_PER_PIXEL>()
            .0
            .iter()
            .zip(gdiplus.as_chunks::<BYTES_PER_PIXEL>().0)
            .filter(|(ours, gdiplus)| ours != gdiplus)
            .count();
        let mean_squared_error = squared_error / ours.len() as f64;
        let psnr_db = if mean_squared_error == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (MAX_CHANNEL * MAX_CHANNEL / mean_squared_error).log10()
        };
        Self {
            differing_pixels,
            max_difference,
            psnr_db,
        }
    }

    fn is_exact(self) -> bool {
        self.differing_pixels == 0
    }

    fn psnr_for_mean(self) -> f64 {
        self.psnr_db.min(EXACT_PSNR_DB)
    }

    fn psnr_text(self) -> String {
        if self.is_exact() {
            "exact".to_string()
        } else {
            format!("{:.2} dB", self.psnr_db)
        }
    }
}

/// One GDI+ reference file: the input it came from and the size GDI+ produced.
struct Reference {
    input_stem: String,
    width: u32,
    height: u32,
    argb: Vec<u8>,
}

impl Reference {
    fn label(&self) -> String {
        format!("{} to {}x{}", self.input_stem, self.width, self.height)
    }
}

fn to_argb(image: &RgbaImage) -> Vec<u8> {
    image
        .pixels()
        .flat_map(|pixel| {
            let [red, green, blue, alpha] = pixel.0;
            [alpha, red, green, blue]
        })
        .collect()
}

fn powershell_32_bit() -> Result<PathBuf> {
    let windows = std::env::var_os(WINDOWS_ROOT_VARIABLE)
        .with_context(|| format!("{WINDOWS_ROOT_VARIABLE} is not set"))?;
    Ok(POWERSHELL_32_BIT
        .iter()
        .fold(PathBuf::from(windows), |path, part| path.join(part)))
}

fn run_reference_tool(paths: &Paths, input_dir: &Path, output_dir: &Path) -> Result<()> {
    let script = REFERENCE_SCRIPT
        .iter()
        .fold(paths.root().to_path_buf(), |path, part| path.join(part));
    let output = Command::new(powershell_32_bit()?)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-InputDir")
        .arg(input_dir)
        .arg("-OutputDir")
        .arg(output_dir)
        .output()
        .context("cannot start 32-bit Windows PowerShell")?;
    if !output.status.success() {
        bail!(
            "the GDI+ reference tool failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn sorted_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("cannot read {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn file_stem(path: &Path) -> Result<String> {
    Ok(path
        .file_stem()
        .with_context(|| format!("{} has no file name", path.display()))?
        .to_string_lossy()
        .into_owned())
}

/// Parses `<input stem>.<width>x<height>.argb`.
fn read_reference(path: &Path) -> Result<Reference> {
    let name = file_stem(path)?;
    let (input_stem, size) = name
        .rsplit_once('.')
        .with_context(|| format!("{name} has no size"))?;
    let (width, height) = size
        .split_once('x')
        .with_context(|| format!("{size} is not <width>x<height>"))?;
    Ok(Reference {
        input_stem: input_stem.to_string(),
        width: width.parse()?,
        height: height.parse()?,
        argb: std::fs::read(path)?,
    })
}

fn read_references(directory: &Path) -> Result<Vec<Reference>> {
    sorted_files(directory)?
        .iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == REFERENCE_EXTENSION)
        })
        .map(|path| read_reference(path))
        .collect()
}

/// Decodes every input once, keyed by file stem.
fn decode_inputs(directory: &Path) -> Result<BTreeMap<String, RgbaImage>> {
    let mut decoded = BTreeMap::new();
    for path in sorted_files(directory)? {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let bytes = std::fs::read(&path)?;
        let image = decode_image(&bytes, file_name.as_deref())
            .with_context(|| format!("cannot decode {}", path.display()))?;
        decoded.insert(file_stem(&path)?, image);
    }
    Ok(decoded)
}

fn input_for<'a>(
    inputs: &'a BTreeMap<String, RgbaImage>,
    reference: &Reference,
) -> Result<&'a RgbaImage> {
    inputs
        .get(&reference.input_stem)
        .with_context(|| format!("no input for {}", reference.label()))
}

/// What the pipeline hands the encoder when `resampler` stretches `image` to the reference size.
fn pipeline_argb(image: &RgbaImage, reference: &Reference, resampler: Resampler) -> Vec<u8> {
    let fitted = if image.dimensions() == (reference.width, reference.height) {
        image.clone()
    } else {
        resampler.resize(image, reference.width, reference.height)
    };
    to_argb(&encoder_pixels(&fitted))
}

struct GateRow {
    label: String,
    pipeline: Closeness,
    without_copy: Closeness,
    semi_transparent_pixels: usize,
}

fn measure_gate(
    inputs: &BTreeMap<String, RgbaImage>,
    references: &[Reference],
) -> Result<Vec<GateRow>> {
    references
        .iter()
        .map(|reference| {
            let image = input_for(inputs, reference)?;
            let semi_transparent_pixels = image
                .pixels()
                .filter(|pixel| pixel[3] != 0 && pixel[3] != u8::MAX)
                .count();
            Ok(GateRow {
                label: reference.label(),
                pipeline: Closeness::measure(
                    &pipeline_argb(image, reference, RESIZE_FILTER),
                    &reference.argb,
                ),
                without_copy: Closeness::measure(&to_argb(image), &reference.argb),
                semi_transparent_pixels,
            })
        })
        .collect()
}

struct ResizeRow {
    label: String,
    has_alpha: bool,
    per_resampler: Vec<Closeness>,
}

fn measure_resize(
    inputs: &BTreeMap<String, RgbaImage>,
    references: &[Reference],
) -> Result<Vec<ResizeRow>> {
    references
        .iter()
        .map(|reference| {
            let image = input_for(inputs, reference)?;
            let per_resampler = Resampler::ALL
                .iter()
                .map(|&resampler| {
                    Closeness::measure(&pipeline_argb(image, reference, resampler), &reference.argb)
                })
                .collect();
            Ok(ResizeRow {
                label: reference.label(),
                has_alpha: image.pixels().any(|pixel| pixel[3] != u8::MAX),
                per_resampler,
            })
        })
        .collect()
}

fn mean_psnr<'a>(rows: impl Iterator<Item = &'a ResizeRow>, resampler_index: usize) -> f64 {
    let values: Vec<f64> = rows
        .map(|row| row.per_resampler[resampler_index].psnr_for_mean())
        .collect();
    values.iter().sum::<f64>() / values.len() as f64
}

fn chosen_resampler(rows: &[ResizeRow]) -> Resampler {
    let mut best = (Resampler::ALL[0], f64::NEG_INFINITY);
    for (index, &resampler) in Resampler::ALL.iter().enumerate() {
        let mean = mean_psnr(rows.iter(), index);
        if mean > best.1 {
            best = (resampler, mean);
        }
    }
    best.0
}

/// Alpha-0 pixels in the GDI+ resize references, and how many of them keep a colour.
fn transparent_colour_counts(references: &[Reference]) -> (usize, usize) {
    let mut transparent = 0;
    let mut coloured = 0;
    for reference in references {
        for pixel in reference.argb.as_chunks::<BYTES_PER_PIXEL>().0 {
            if pixel[ALPHA] == 0 {
                transparent += 1;
                if pixel[1..].iter().any(|&channel| channel != 0) {
                    coloured += 1;
                }
            }
        }
    }
    (transparent, coloured)
}

/// The committed gate inputs must be exactly what `closeness-inputs` writes today.
fn ensure_gate_inputs_current(committed_dir: &Path, regenerated_dir: &Path) -> Result<()> {
    let committed = sorted_files(committed_dir)?;
    let regenerated = sorted_files(regenerated_dir)?;
    let names = |files: &[PathBuf]| -> Vec<String> {
        files
            .iter()
            .map(|file| {
                file.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect()
    };
    ensure!(
        names(&committed) == names(&regenerated),
        "the committed gate inputs {:?} differ from the generated set {:?}",
        names(&committed),
        names(&regenerated)
    );
    for (committed, regenerated) in committed.iter().zip(&regenerated) {
        ensure!(
            std::fs::read(committed)? == std::fs::read(regenerated)?,
            "the committed gate input {} differs from `cargo xtask closeness-inputs`",
            committed.display()
        );
    }
    Ok(())
}

fn ensure_references_current(committed: &[Reference], regenerated: &[Reference]) -> Result<()> {
    ensure!(
        committed.len() == regenerated.len(),
        "{} committed GDI+ gate references, the tool wrote {}",
        committed.len(),
        regenerated.len()
    );
    for (committed, regenerated) in committed.iter().zip(regenerated) {
        ensure!(
            committed.label() == regenerated.label() && committed.argb == regenerated.argb,
            "the committed GDI+ reference {} differs from a fresh run of the tool",
            committed.label()
        );
    }
    Ok(())
}

fn write_gate_section(report: &mut String, rows: &[GateRow]) -> Result<()> {
    let exact = rows.iter().filter(|row| row.pipeline.is_exact()).count();
    writeln!(report, "## Gate: PNG and BMP at target size\n")?;
    writeln!(
        report,
        "Gate result: {exact} of {} inputs match GDI+ byte for byte (`png_and_bmp_at_target_size_match_gdiplus`).\n",
        rows.len()
    )?;
    writeln!(
        report,
        "\"Straight decode\" compares the decoded pixels without the premultiplied copy that `ImageToRawArgb` makes with `new Bitmap(img)`; the difference is what that copy changes.\n"
    )?;
    writeln!(
        report,
        "| Input | Pipeline | Straight decode: differing pixels | Straight decode: max difference | Semi-transparent pixels |"
    )?;
    writeln!(report, "|---|---|---|---|---|")?;
    for row in rows {
        writeln!(
            report,
            "| {} | {} | {} | {} | {} |",
            row.label,
            row.pipeline.psnr_text(),
            row.without_copy.differing_pixels,
            row.without_copy.max_difference,
            row.semi_transparent_pixels
        )?;
    }
    writeln!(report)?;
    Ok(())
}

fn write_summary_table(report: &mut String, rows: &[ResizeRow]) -> Result<()> {
    let alpha_rows = || rows.iter().filter(|row| row.has_alpha);
    writeln!(
        report,
        "| Filter | Mean PSNR | Mean PSNR, inputs with alpha | Mean PSNR, opaque inputs | Worst max difference |"
    )?;
    writeln!(report, "|---|---|---|---|---|")?;
    for (index, resampler) in Resampler::ALL.iter().enumerate() {
        let worst = rows
            .iter()
            .map(|row| row.per_resampler[index].max_difference)
            .max()
            .unwrap_or(0);
        writeln!(
            report,
            "| {} | {:.2} dB | {:.2} dB | {:.2} dB | {worst} |",
            resampler.name(),
            mean_psnr(rows.iter(), index),
            mean_psnr(alpha_rows(), index),
            mean_psnr(rows.iter().filter(|row| !row.has_alpha), index),
        )?;
    }
    writeln!(report)?;
    Ok(())
}

fn write_resize_section(
    report: &mut String,
    rows: &[ResizeRow],
    transparent_colours: (usize, usize),
) -> Result<()> {
    writeln!(report, "## Resize set\n")?;
    writeln!(
        report,
        "Each input is decoded, stretched to every target with each candidate filter, passed through the encoder copy, and compared with GDI+'s `new Bitmap(img, size)` followed by `ImageToRawArgb`. Means count an exact match as {EXACT_PSNR_DB:.0} dB.\n"
    )?;
    writeln!(
        report,
        "The `image` crate filters widen their kernel when shrinking; \"(premultiplied)\" runs them on premultiplied alpha. \"Bilinear\" is two taps at any scale, centres aligned, straight alpha. \"GDI+ bilinear\" reproduces GDI+'s default `DrawImage`: premultiplied, two taps with pixel origins aligned and transparent black past the right and bottom edges, and area averaging on shrinking axes when the source is 420 pixels or more on a side. JPEG inputs also carry the decoding difference below.\n"
    )?;
    write_summary_table(report, rows)?;
    let (transparent, coloured) = transparent_colours;
    writeln!(
        report,
        "Alpha-0 pixels in the GDI+ resize output: {transparent}, of which {coloured} keep a colour other than black.\n"
    )?;
    writeln!(
        report,
        "PSNR and max channel difference per input and target:\n"
    )?;
    let header: Vec<&str> = Resampler::ALL
        .iter()
        .map(|resampler| resampler.name())
        .collect();
    writeln!(report, "| Input | {} |", header.join(" | "))?;
    writeln!(report, "|---|{}", "---|".repeat(header.len()))?;
    for row in rows {
        let cells: Vec<String> = row
            .per_resampler
            .iter()
            .map(|closeness| format!("{} / {}", closeness.psnr_text(), closeness.max_difference))
            .collect();
        writeln!(report, "| {} | {} |", row.label, cells.join(" | "))?;
    }
    writeln!(report)?;
    Ok(())
}

fn write_jpeg_section(report: &mut String, rows: &[(String, Closeness)]) -> Result<()> {
    writeln!(report, "## JPEG decoding\n")?;
    writeln!(
        report,
        "The photo-like scene at cover size, decoded at its own size (no resize).\n"
    )?;
    writeln!(
        report,
        "| Input | PSNR | Max difference | Differing pixels |"
    )?;
    writeln!(report, "|---|---|---|---|")?;
    for (label, closeness) in rows {
        writeln!(
            report,
            "| {label} | {} | {} | {} |",
            closeness.psnr_text(),
            closeness.max_difference,
            closeness.differing_pixels
        )?;
    }
    writeln!(report)?;
    Ok(())
}

fn measure_jpeg(
    inputs: &BTreeMap<String, RgbaImage>,
    references: &[Reference],
) -> Result<Vec<(String, Closeness)>> {
    references
        .iter()
        .map(|reference| {
            let image = input_for(inputs, reference)?;
            let ours = pipeline_argb(image, reference, RESIZE_FILTER);
            Ok((
                reference.label(),
                Closeness::measure(&ours, &reference.argb),
            ))
        })
        .collect()
}

fn generate_set(
    paths: &Paths,
    work: &Path,
    name: &str,
    write_inputs: impl FnOnce(&Path) -> Result<()>,
) -> Result<(BTreeMap<String, RgbaImage>, Vec<Reference>)> {
    let input_dir = work.join(format!("{name}-inputs"));
    let output_dir = work.join(format!("{name}-gdiplus"));
    std::fs::create_dir_all(&input_dir)?;
    write_inputs(&input_dir)?;
    run_reference_tool(paths, &input_dir, &output_dir)?;
    Ok((decode_inputs(&input_dir)?, read_references(&output_dir)?))
}

pub fn write_report() -> Result<()> {
    ensure!(cfg!(windows), "the GDI+ report needs Windows");
    let paths = Paths::new()?;
    let work_dir = tempfile::tempdir()?;
    let work = absolute(work_dir.path())?;

    let gate_inputs = gate_directory(&paths);
    let gate_references = read_references(&paths.closeness().join(REFERENCE_DIRECTORY))?;
    let fresh_gate_inputs = work.join("gate-inputs");
    std::fs::create_dir_all(&fresh_gate_inputs)?;
    closeness_inputs::write_gate_set(&paths, &fresh_gate_inputs)?;
    ensure_gate_inputs_current(&gate_inputs, &fresh_gate_inputs)?;
    let fresh_gate_dir = work.join("gate-gdiplus");
    run_reference_tool(&paths, &fresh_gate_inputs, &fresh_gate_dir)?;
    ensure_references_current(&gate_references, &read_references(&fresh_gate_dir)?)?;
    let gate_rows = measure_gate(&decode_inputs(&gate_inputs)?, &gate_references)?;

    let (resize_inputs, resize_references) = generate_set(&paths, &work, "resize", |dir| {
        closeness_inputs::write_resize_set(&paths, dir)
    })?;
    let resize_rows = measure_resize(&resize_inputs, &resize_references)?;
    let (jpeg_inputs, jpeg_references) =
        generate_set(&paths, &work, "jpeg", closeness_inputs::write_jpeg_set)?;
    let jpeg_rows = measure_jpeg(&jpeg_inputs, &jpeg_references)?;

    let chosen = chosen_resampler(&resize_rows);
    let mut report = String::new();
    writeln!(report, "# Closeness to GDI+\n")?;
    writeln!(
        report,
        "Generated by `cargo xtask gdiplus-report` (spec 6.2); do not edit. GDI+ runs the old app's path, `MainWindow.GetImage` then `AuroraAsset.ImageToRawArgb`, through `tools/gdiplus/gdiplus-reference.ps1` in 32-bit Windows PowerShell 5.1. All numbers compare A, R, G, B bytes.\n"
    )?;
    writeln!(report, "Chosen filter: {}\n", chosen.name())?;
    write_gate_section(&mut report, &gate_rows)?;
    write_resize_section(
        &mut report,
        &resize_rows,
        transparent_colour_counts(&resize_references),
    )?;
    write_jpeg_section(&mut report, &jpeg_rows)?;

    let report_path = REPORT_PATH
        .iter()
        .fold(paths.root().to_path_buf(), |path, part| path.join(part));
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, report)
        .with_context(|| format!("cannot write {}", report_path.display()))?;
    println!("{}: chosen filter {}", report_path.display(), chosen.name());
    Ok(())
}
