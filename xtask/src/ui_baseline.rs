use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::release_tags::{latest_v2_tag, repository_tags};
use crate::ui_review_zip::{MANIFEST_FILE, UI_SCREENSHOTS_ZIP, extract_zip};
use crate::ui_screenshots::{BASELINE_VARIABLE, current_platform, repository_root};

pub const USAGE_LINE: &str = "cargo xtask ui-baseline <dest> --env-file <file>";

/// The environment line that points the screenshot suite at this platform's earlier run, or
/// `None` when the unpacked release holds no run of this platform.
fn baseline_env_line(unpacked: &Path, platform: &str) -> Option<String> {
    let folder = unpacked.join(platform);
    if !folder.join(MANIFEST_FILE).is_file() {
        return None;
    }
    Some(format!("{BASELINE_VARIABLE}={}", folder.display()))
}

/// Downloads the screenshots of the previous v2 release into `<dest>/unpacked` and appends the
/// baseline line for this platform to `env_file`.
pub fn run(dest: &str, env_file: &str) -> Result<()> {
    let root = repository_root();
    let tags = repository_tags(&root)?;
    let Some(tag) = latest_v2_tag(&tags) else {
        println!("No earlier v2 release: every image is new");
        return Ok(());
    };

    let working_folder = std::env::current_dir().context("could not read the current folder")?;
    let dest = working_folder.join(dest);
    let download = dest.join("download");
    download_screenshots(&root, &tag, &download)?;
    let unpacked = dest.join("unpacked");
    extract_zip(&download.join(UI_SCREENSHOTS_ZIP), &unpacked)
        .with_context(|| format!("could not unpack the screenshots of {tag}"))?;

    let platform = current_platform();
    match baseline_env_line(&unpacked, &platform) {
        Some(line) => append_line(&working_folder.join(env_file), &line),
        None => {
            println!("{tag} has no {platform} screenshots: every image is new");
            Ok(())
        }
    }
}

fn download_screenshots(root: &Path, tag: &str, download: &Path) -> Result<()> {
    let output = Command::new("gh")
        .current_dir(root)
        .args([
            "release",
            "download",
            tag,
            "--pattern",
            UI_SCREENSHOTS_ZIP,
            "--dir",
        ])
        .arg(download)
        .output()
        .context("could not run `gh release download`")?;
    if !output.status.success() {
        bail!(
            "could not download {UI_SCREENSHOTS_ZIP} of release {tag}: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn append_line(env_file: &Path, line: &str) -> Result<()> {
    if let Some(parent) = env_file.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env_file)
        .with_context(|| format!("could not open {}", env_file.display()))?;
    writeln!(file, "{line}").with_context(|| format!("could not write {}", env_file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_line_names_the_platform_folder() {
        let unpacked = tempfile::tempdir().unwrap();
        let folder = unpacked.path().join("linux-x86_64");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(MANIFEST_FILE), "manifest").unwrap();

        assert_eq!(
            baseline_env_line(unpacked.path(), "linux-x86_64"),
            Some(format!("AURORA_UI_BASELINE={}", folder.display()))
        );
        assert_eq!(baseline_env_line(unpacked.path(), "macos-aarch64"), None);
    }
}
