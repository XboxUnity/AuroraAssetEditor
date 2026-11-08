use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::ui_screenshots::{EDITOR_PACKAGE, SCREENSHOT_TEST_TARGET};

// The variable name and the text below mirror `editor-test-support`, which xtask does not depend on.
const ASSETS_VARIABLE: &str = "AURORA_TEST_ASSETS";
const NOT_SET_TEXT: &str = "AURORA_TEST_ASSETS is not set. The README screenshots show real game art from a \
     folder kept outside the repository: set AURORA_TEST_ASSETS to a folder holding \
     GameData/<TitleID>_<DatabaseID>/ and DataBases/content.db, pulled from an Aurora console.";

// The test name and the folder mirror the `ui_screenshots` test target and `docs/images/readme`.
const README_IMAGE_TEST: &str = "render_readme_images";
const README_IMAGES_FOLDER: &str = "docs/images/readme";

pub const USAGE_LINE: &str = "cargo xtask readme-screenshots";

/// Renders the README images from the user's real assets and lists what changed in the image
/// folder, so the user can review and commit them.
pub fn run() -> Result<()> {
    check_asset_folder(std::env::var_os(ASSETS_VARIABLE).as_deref())?;
    let root = repository_root();

    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "test",
            "-p",
            EDITOR_PACKAGE,
            "--test",
            SCREENSHOT_TEST_TARGET,
        ])
        .args([
            "--",
            "--ignored",
            "--exact",
            README_IMAGE_TEST,
            "--nocapture",
        ])
        .status()
        .context("could not run `cargo test` for the README images")?;
    if !status.success() {
        bail!("rendering the README images failed: `cargo test` exited with {status}");
    }

    let status = Command::new("git")
        .current_dir(&root)
        .args(["status", "--short", "--", README_IMAGES_FOLDER])
        .status()
        .context("could not run `git status` for the README images")?;
    if !status.success() {
        bail!("`git status` exited with {status}");
    }
    Ok(())
}

fn check_asset_folder(value: Option<&OsStr>) -> Result<PathBuf> {
    match value {
        Some(value) if !value.is_empty() => {
            let folder = PathBuf::from(value);
            if !folder.is_dir() {
                bail!(
                    "{ASSETS_VARIABLE} points to {}, which is not a folder",
                    folder.display()
                );
            }
            Ok(folder)
        }
        _ => bail!("{NOT_SET_TEXT}"),
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the repository root")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_asset_folder_names_the_variable() {
        for value in [None, Some(OsStr::new(""))] {
            let message = check_asset_folder(value)
                .expect_err("an unset variable is refused")
                .to_string();
            assert!(message.contains("AURORA_TEST_ASSETS"), "{message}");
            assert!(message.contains("DataBases/content.db"), "{message}");
        }
    }

    #[test]
    fn asset_folder_that_is_not_a_folder_is_refused() {
        let file = tempfile::NamedTempFile::new().expect("a temp file");
        let message = check_asset_folder(Some(file.path().as_os_str()))
            .expect_err("a file is not an asset folder")
            .to_string();
        assert!(message.contains("AURORA_TEST_ASSETS"), "{message}");
        assert!(
            message.contains(&file.path().display().to_string()),
            "{message}"
        );
    }

    #[test]
    fn asset_folder_is_accepted() {
        let folder = tempfile::tempdir().expect("a temp folder");
        assert_eq!(
            check_asset_folder(Some(folder.path().as_os_str()))
                .expect("an existing folder is accepted"),
            folder.path()
        );
    }
}
