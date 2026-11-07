use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

use super::{BYTES_PER_PIXEL, Paths, bgra_to_rgba};

pub(super) const HARNESS_TARGET: &str = "i686-pc-windows-msvc";
const HARNESS_PACKAGE: &str = "golden-harness";
const HARNESS_ENCODE_REFUSED_EXIT_CODE: i32 = 4;
const HARNESS_DECODE_REFUSED_EXIT_CODE: i32 = 5;

fn cargo_command() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

pub(crate) fn build_harness(paths: &Paths) -> Result<()> {
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

/// Runs the harness; `None` means the DLL refused the input, any other failure is an error.
pub(crate) fn run_harness_allowing_refusal(
    paths: &Paths,
    arguments: &[OsString],
) -> Result<Option<String>> {
    let output = Command::new(paths.harness())
        .args(arguments)
        .output()
        .context("cannot start the golden harness")?;
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()));
    }
    let refused = matches!(
        output.status.code(),
        Some(HARNESS_ENCODE_REFUSED_EXIT_CODE | HARNESS_DECODE_REFUSED_EXIT_CODE)
    );
    if refused {
        return Ok(None);
    }
    bail!(
        "golden harness failed (exit status {}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

pub(crate) fn run_harness(paths: &Paths, arguments: &[OsString]) -> Result<String> {
    run_harness_allowing_refusal(paths, arguments)?.context("the DLL refused the input")
}

pub(crate) fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// The DLL's RGBA decode of a texture, or `None` when it refuses it.
pub(crate) fn dll_decode(
    paths: &Paths,
    work_dir: &Path,
    label: &str,
    header: &[u8],
    video: &[u8],
) -> Result<Option<Vec<u8>>> {
    let header_path = work_dir.join("decode.header");
    let video_path = work_dir.join("decode.video");
    let decoded_path = work_dir.join("decode.bgra");
    std::fs::write(&header_path, header)?;
    std::fs::write(&video_path, video)?;
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
    .with_context(|| format!("decoding {label}"))?;
    let Some(report) = report else {
        return Ok(None);
    };
    let bgra = std::fs::read(&decoded_path)?;
    let reported_pixels = report
        .split_whitespace()
        .map(str::parse::<usize>)
        .product::<Result<usize, _>>()
        .with_context(|| format!("{label}: unreadable decode report {:?}", report.trim()))?;
    ensure!(
        bgra.len() == reported_pixels * BYTES_PER_PIXEL,
        "{label}: decode report {:?} does not match {} output bytes",
        report.trim(),
        bgra.len()
    );
    Ok(Some(bgra_to_rgba(&bgra)))
}
