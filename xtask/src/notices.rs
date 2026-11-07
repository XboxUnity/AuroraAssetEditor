use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const EDITOR_CRATE: &str = "crates/aurora-asset-editor";
const NOTICES_FOLDER: &str = "crates/aurora-asset-editor/notices";
const CONFIG_FILE: &str = "about.toml";
const TEMPLATE_FILE: &str = "about.hbs";
const OUTPUT_FILE: &str = "third-party.txt";
const HANDWRITTEN_FILE: &str = "handwritten.txt";
const SLINT_LICENSE: &str = "LicenseRef-Slint-Royalty-free-2.0";
const SLINT_CRATES: [&str; 9] = [
    "i-slint-backend-selector",
    "i-slint-backend-winit",
    "i-slint-common",
    "i-slint-compiler",
    "i-slint-core",
    "i-slint-core-macros",
    "i-slint-renderer-software",
    "slint",
    "slint-macros",
];

/// Writes `third-party.txt` with `cargo about`. With `check` it only compares the result with the file in the
/// repository and fails on any difference.
pub fn run(check: bool) -> Result<()> {
    let root = repository_root();
    let notices = root.join(NOTICES_FOLDER);
    let committed_path = notices.join(OUTPUT_FILE);
    if !check {
        return generate(&root, &notices, &committed_path);
    }
    let scratch = tempfile::tempdir().context("could not create a temporary folder")?;
    let regenerated_path = scratch.path().join(OUTPUT_FILE);
    generate(&root, &notices, &regenerated_path)?;
    check_slint_text(&root, &notices)?;
    let committed = read_normalized(&committed_path)?;
    let regenerated = read_normalized(&regenerated_path)?;
    if committed != regenerated {
        bail!("{OUTPUT_FILE} is out of date, run `cargo xtask notices` and commit the result");
    }
    println!("{OUTPUT_FILE} is up to date");
    Ok(())
}

fn generate(root: &Path, notices: &Path, output: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .arg("about")
        .arg("generate")
        .arg("--config")
        .arg(notices.join(CONFIG_FILE))
        .arg("--manifest-path")
        .arg(root.join(EDITOR_CRATE).join("Cargo.toml"))
        .arg("--output-file")
        .arg(output)
        .arg(notices.join(TEMPLATE_FILE))
        .output()
        .context("could not run `cargo about`; install it with `cargo install cargo-about --version 0.9.3 --locked --features cli`")?;
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if !is_expected_warning(line) {
            eprintln!("{line}");
        }
    }
    if !output.status.success() {
        bail!("`cargo about generate` failed with {}", output.status);
    }
    Ok(())
}

/// cargo-about cannot find the Slint royalty-free licence text for the Slint crates, so it leaves them out of the
/// generated file; `handwritten.txt` carries that text instead (see `check_slint_text`). Every other warning is
/// printed.
fn is_expected_warning(line: &str) -> bool {
    line.contains(SLINT_LICENSE)
        && line.contains("has no license file for crate")
        && SLINT_CRATES
            .iter()
            .any(|name| line.contains(&format!("'{name} ")))
}

/// `handwritten.txt` must hold the Slint licence exactly as the `slint` crate in use ships it.
fn check_slint_text(root: &Path, notices: &Path) -> Result<()> {
    let metadata = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(root.join(EDITOR_CRATE).join("Cargo.toml"))
        .output()
        .context("could not run `cargo metadata`")?;
    if !metadata.status.success() {
        bail!("`cargo metadata` failed with {}", metadata.status);
    }
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout)
        .context("`cargo metadata` printed invalid JSON")?;
    let manifest = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|package| package["name"] == "slint"))
        .and_then(|package| package["manifest_path"].as_str())
        .context("the dependency tree has no `slint` package")?;
    let licence_path = Path::new(manifest)
        .parent()
        .context("the slint manifest has no folder")?
        .join("LICENSES")
        .join(format!("{SLINT_LICENSE}.md"));
    let licence = read_normalized(&licence_path)?;
    let handwritten = read_normalized(&notices.join(HANDWRITTEN_FILE))?;
    if !handwritten.contains(licence.trim()) {
        bail!(
            "{HANDWRITTEN_FILE} does not hold the text of {}",
            licence_path.display()
        );
    }
    Ok(())
}

/// The text with Windows line endings turned into `\n`, so the check does not depend on the platform.
fn read_normalized(path: &Path) -> Result<String> {
    let text =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    Ok(text.replace("\r\n", "\n"))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the repository root")
        .to_path_buf()
}
