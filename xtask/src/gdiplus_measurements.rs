//! The report's section on how GDI+ itself behaves: the premultiplied copy, 16-bit PNG samples,
//! the area-averaging threshold, 32-bit against 64-bit hosts, and the grey table.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use aurora_image::{decode_image, encoder_pixels};
use image::{DynamicImage, Rgba, RgbaImage};

use crate::closeness_inputs::{self, RAMP_SIDE, gate_directory};
use crate::gdiplus_report::{
    Closeness, REFERENCE_DIRECTORY, Reference, file_stem, generate_set, measure_resize,
    sorted_files, to_argb, write_per_input_table,
};
use crate::gdiplus_tool::{PowerShellHost, run_reference_tool};
use crate::golden::Paths;
use crate::gray16_table::{measure_table_source, table_path};

const BYTE_BITS: u16 = 8;

/// One ramp compared with GDI+ several ways, each through the encoder copy unless noted.
struct RampRow {
    label: String,
    pipeline: Closeness,
    high_byte: Option<Closeness>,
    rounded: Closeness,
    without_copy: Closeness,
}

fn high_bytes(image: &DynamicImage) -> RgbaImage {
    let wide = image.to_rgba16();
    RgbaImage::from_fn(wide.width(), wide.height(), |x, y| {
        Rgba(
            wide.get_pixel(x, y)
                .0
                .map(|sample| (sample >> BYTE_BITS) as u8),
        )
    })
}

fn is_sixteen_bit(image: &DynamicImage) -> bool {
    image.color().bytes_per_pixel() > image.color().channel_count()
}

fn measure_ramp(input: &Path, reference: &Reference) -> Result<RampRow> {
    let bytes = std::fs::read(input)?;
    let file_name = input
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let decoded = decode_image(&bytes, file_name.as_deref())
        .with_context(|| format!("cannot decode {}", input.display()))?;
    let plain = image::load_from_memory(&bytes)?;
    let compare = |image: &RgbaImage| Closeness::measure(&to_argb(image), &reference.argb);
    Ok(RampRow {
        label: reference.input_stem.clone(),
        pipeline: compare(&encoder_pixels(&decoded)),
        high_byte: is_sixteen_bit(&plain).then(|| compare(&encoder_pixels(&high_bytes(&plain)))),
        rounded: compare(&encoder_pixels(&plain.to_rgba8())),
        without_copy: compare(&decoded),
    })
}

fn measure_ramps(input_dir: &Path, references: &[Reference]) -> Result<Vec<RampRow>> {
    let inputs = sorted_files(input_dir)?;
    references
        .iter()
        .map(|reference| {
            let input = inputs
                .iter()
                .find(|input| file_stem(input).is_ok_and(|stem| stem == reference.input_stem))
                .with_context(|| format!("no input for {}", reference.label()))?;
            measure_ramp(input, reference)
        })
        .collect()
}

fn write_ramp_table(report: &mut String, rows: &[RampRow]) -> Result<()> {
    writeln!(
        report,
        "| Ramp | Pipeline | High byte | Rounded (`image`) | Pipeline without the copy |"
    )?;
    writeln!(report, "|---|---|---|---|---|")?;
    for row in rows {
        let high_byte = row
            .high_byte
            .map_or("8-bit input".to_string(), |closeness| {
                closeness.differing_pixels.to_string()
            });
        writeln!(
            report,
            "| {} | {} | {high_byte} | {} | {} |",
            row.label,
            row.pipeline.differing_pixels,
            row.rounded.differing_pixels,
            row.without_copy.differing_pixels
        )?;
    }
    writeln!(report)?;
    Ok(())
}

/// Files the 32-bit and 64-bit hosts wrote with identical bytes, and the number compared.
fn identical_host_files(
    paths: &Paths,
    work: &Path,
    sets: &[(&str, &Path, &Path, Option<&str>)],
) -> Result<(usize, usize)> {
    let mut identical = 0;
    let mut compared = 0;
    for &(name, input_dir, bit32_dir, targets) in sets {
        let bit64_dir = work.join(format!("{name}-gdiplus64"));
        run_reference_tool(paths, PowerShellHost::Bit64, input_dir, &bit64_dir, targets)?;
        let bit32 = sorted_files(bit32_dir)?;
        let bit64 = sorted_files(&bit64_dir)?;
        ensure!(
            bit32.len() == bit64.len(),
            "the 64-bit host wrote {} {name} references, the 32-bit host {}",
            bit64.len(),
            bit32.len()
        );
        for (left, right) in bit32.iter().zip(&bit64) {
            compared += 1;
            if left.file_name() == right.file_name()
                && std::fs::read(left)? == std::fs::read(right)?
            {
                identical += 1;
            }
        }
    }
    Ok((identical, compared))
}

fn ensure_grey_table_current(paths: &Paths, work: &Path) -> Result<()> {
    let committed_path = table_path(paths);
    let committed = std::fs::read_to_string(&committed_path)
        .with_context(|| format!("cannot read {}", committed_path.display()))?;
    ensure!(
        committed == measure_table_source(paths, work)?,
        "{} differs from GDI+; run `cargo xtask gray16-table`",
        committed_path.display()
    );
    Ok(())
}

pub fn write_measurements_section(report: &mut String, paths: &Paths, work: &Path) -> Result<()> {
    let side = format!("{RAMP_SIDE}x{RAMP_SIDE}");
    let (_, ramp_references) = generate_set(
        paths,
        work,
        "ramp",
        closeness_inputs::write_ramp_set,
        Some(&side),
    )?;
    let ramp_rows = measure_ramps(&work.join("ramp-inputs"), &ramp_references)?;
    let (threshold_inputs, threshold_references) = generate_set(
        paths,
        work,
        "threshold",
        closeness_inputs::write_threshold_set,
        None,
    )?;
    let threshold_rows = measure_resize(&threshold_inputs, &threshold_references)?;
    let gate_inputs = gate_directory(paths);
    let gate_references = paths.closeness().join(REFERENCE_DIRECTORY);
    let ramp_inputs = work.join("ramp-inputs");
    let ramp_gdiplus = work.join("ramp-gdiplus");
    let (identical, compared) = identical_host_files(
        paths,
        work,
        &[
            ("gate", &gate_inputs, &gate_references, None),
            ("ramp", &ramp_inputs, &ramp_gdiplus, Some(&side)),
        ],
    )?;
    ensure_grey_table_current(paths, work)?;

    writeln!(report, "## Measured GDI+ behaviour\n")?;
    writeln!(report, "### Premultiplied copy and 16-bit samples\n")?;
    writeln!(
        report,
        "Ramps of {side} pixels, measured at their own size; each cell counts the pixels of 65536 that differ from GDI+. \"Pipeline\" is `decode_image` and `encoder_pixels`. \"High byte\" and \"Rounded\" reduce 16-bit samples by `v >> 8` or with `image`'s rounding, then apply the copy. \"Pipeline without the copy\" leaves out `encoder_pixels`, which is how many pixels `new Bitmap(img)` changes. `colour_alpha_ramp` holds every (colour, alpha) pair of an 8-bit channel, so a 0 in its pipeline column means GDI+'s premultiply and unpremultiply are reproduced exactly.\n"
    )?;
    write_ramp_table(report, &ramp_rows)?;
    writeln!(
        report,
        "The committed grey table `crates/aurora-image/src/gdiplus_gray16_table.rs` matches a fresh measurement of `gray16_ramp`.\n"
    )?;
    writeln!(report, "### Area-averaging threshold\n")?;
    writeln!(
        report,
        "Opaque noise 419 and 420 pixels wide, stretched to every target. GDI+ resamples the 419-pixel source bilinearly and averages areas on the shrinking axes of the 420-pixel one; \"GDI+ bilinear\" switches at 420.\n"
    )?;
    write_per_input_table(report, &threshold_rows)?;
    writeln!(report, "### 32-bit and 64-bit hosts\n")?;
    writeln!(
        report,
        "32-bit and 64-bit Windows PowerShell 5.1 wrote identical references for {identical} of {compared} files (gate and ramp sets).\n"
    )?;
    Ok(())
}
