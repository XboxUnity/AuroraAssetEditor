use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

// The same names and paths as in `editor-test-support`, which xtask does not depend on.
const REFERENCES_VARIABLE: &str = "AURORA_UI_REFERENCES";
const SCREENS_VARIABLE: &str = "AURORA_UI_SCREENS";
const CONTACT_SHEET: &str = "target/ui-screenshots/index.html";
const REFERENCES_FOLDER: &str = "fixtures/ui";

const EDITOR_PACKAGE: &str = "aurora-asset-editor";
const SCREENSHOT_TEST_TARGET: &str = "ui_screenshots";
/// Characters a `file://` URL carries as they are; every other byte is percent-encoded.
const URL_SAFE_PUNCTUATION: &str = "-._~/:";

/// Whether the run only compares the screens with their references or rewrites the changed ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotMode {
    Check,
    Update,
}

impl ScreenshotMode {
    fn variable_value(self) -> &'static str {
        match self {
            ScreenshotMode::Check => "check",
            ScreenshotMode::Update => "update",
        }
    }
}

/// Runs the screenshot suite, limited to the screens whose name contains `only` when given, and
/// prints where the contact sheet is. An update also lists the reference files it changed.
pub fn run(mode: ScreenshotMode, only: Option<&str>) -> Result<()> {
    if only.is_some_and(str::is_empty) {
        bail!("--only needs the text the screen names must contain; it was empty");
    }
    let root = repository_root();
    let mut test = Command::new("cargo");
    test.current_dir(&root)
        .args([
            "test",
            "-p",
            EDITOR_PACKAGE,
            "--test",
            SCREENSHOT_TEST_TARGET,
        ])
        .env(REFERENCES_VARIABLE, mode.variable_value());
    match only {
        Some(text) => test.env(SCREENS_VARIABLE, text),
        None => test.env_remove(SCREENS_VARIABLE),
    };
    let status = test
        .status()
        .context("could not run `cargo test` for the UI screenshots")?;

    let reference_changes = match mode {
        ScreenshotMode::Update => print_reference_changes(&root),
        ScreenshotMode::Check => Ok(()),
    };
    print_contact_sheet(&root);

    // A failed test run is the main result, so a failing `git status` never hides it.
    if status.success() {
        return reference_changes;
    }
    let test_failure = format!("the UI screenshot tests failed: `cargo test` exited with {status}");
    match reference_changes {
        Ok(()) => bail!("{test_failure}"),
        Err(git_error) => {
            bail!("{test_failure}; listing the changed references failed too: {git_error:#}")
        }
    }
}

/// Every new file is listed by name, also while the folder itself is still untracked.
fn print_reference_changes(root: &Path) -> Result<()> {
    println!("Changed references (git status --short -- {REFERENCES_FOLDER}):");
    let status = Command::new("git")
        .current_dir(root)
        .args([
            "status",
            "--short",
            "--untracked-files=all",
            "--",
            REFERENCES_FOLDER,
        ])
        .status()
        .context("could not run `git status`")?;
    if !status.success() {
        bail!("`git status` failed with {status}");
    }
    Ok(())
}

/// A run that could not write the sheet says so in the test output; here it is only named.
fn print_contact_sheet(root: &Path) {
    let sheet = root.join(CONTACT_SHEET);
    if sheet.is_file() {
        println!("Contact sheet: {}", file_url(&sheet));
    } else {
        println!(
            "Contact sheet: not written ({} is missing)",
            sheet.display()
        );
    }
}

/// `D:\repo\target\index.html` becomes `file:///D:/repo/target/index.html`, which a terminal can
/// open.
fn file_url(path: &Path) -> String {
    let slashed = path.display().to_string().replace('\\', "/");
    let mut url = String::from("file://");
    if !slashed.starts_with('/') {
        url.push('/');
    }
    for byte in slashed.bytes() {
        let character = char::from(byte);
        if character.is_ascii_alphanumeric() || URL_SAFE_PUNCTUATION.contains(character) {
            url.push(character);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the repository root")
        .to_path_buf()
}
