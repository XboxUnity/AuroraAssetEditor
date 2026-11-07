use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const EDITOR_CRATE: &str = "crates/aurora-asset-editor";
const NOTICES_FOLDER: &str = "crates/aurora-asset-editor/notices";
const CONFIG_FILE: &str = "about.toml";
const TEMPLATE_FILE: &str = "about.hbs";
const OUTPUT_FILE: &str = "third-party.txt";

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
    let committed = read_normalized(&committed_path)?;
    let regenerated = read_normalized(&regenerated_path)?;
    if committed != regenerated {
        bail!("{OUTPUT_FILE} is out of date, run `cargo xtask notices` and commit the result");
    }
    println!("{OUTPUT_FILE} is up to date");
    Ok(())
}

fn generate(root: &Path, notices: &Path, output: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .arg("about")
        .arg("generate")
        .arg("--config")
        .arg(notices.join(CONFIG_FILE))
        .arg("--manifest-path")
        .arg(root.join(EDITOR_CRATE).join("Cargo.toml"))
        .arg("--output-file")
        .arg(output)
        .arg(notices.join(TEMPLATE_FILE))
        .status()
        .context("could not run `cargo about`; install it with `cargo install cargo-about --version 0.9.3 --locked --features cli`")?;
    if !status.success() {
        bail!("`cargo about generate` failed with {status}");
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
