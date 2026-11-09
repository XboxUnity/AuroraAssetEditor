use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::asset_names::expected_release_assets;
use crate::golden::sha256_hex;
use crate::set_version::validate_version;

pub const USAGE_LINE: &str = "cargo xtask checksums <version> <folder>";
pub const CHECKSUMS_FILE: &str = "SHA256SUMS.txt";

/// Checks that `folder` holds exactly the release assets of `version` and writes their
/// `SHA256SUMS.txt` beside them.
pub fn run(version: &str, folder: &str) -> Result<()> {
    validate_version(version)?;
    let written = write_checksums(version, Path::new(folder))?;
    println!("Wrote {}", written.display());
    Ok(())
}

/// Writes `<sha256 hex>  <name>` per asset, sorted by name: the `sha256sum` format, so
/// `sha256sum -c` checks a download.
pub fn write_checksums(version: &str, folder: &Path) -> Result<PathBuf> {
    let expected = expected_release_assets(version);
    check_folder_holds_exactly(folder, &expected)?;
    let mut text = String::new();
    for name in &expected {
        let path = folder.join(name);
        let bytes =
            fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
        text.push_str(&format!("{}  {name}\n", sha256_hex(&bytes)));
    }
    let checksums = folder.join(CHECKSUMS_FILE);
    fs::write(&checksums, text)
        .with_context(|| format!("could not write {}", checksums.display()))?;
    Ok(checksums)
}

/// The folder's files, an earlier checksum list aside, must be the expected assets; a
/// subfolder is refused too, since a release attaches only files.
fn check_folder_holds_exactly(folder: &Path, expected: &[String]) -> Result<()> {
    let mut found = Vec::new();
    let listing =
        fs::read_dir(folder).with_context(|| format!("could not read {}", folder.display()))?;
    for entry in listing {
        let path = entry
            .with_context(|| format!("could not read {}", folder.display()))?
            .path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            bail!("{} has a name that is not valid UTF-8", path.display());
        };
        if path.is_dir() {
            bail!(
                "{} holds the folder `{name}`, but a release holds only files",
                folder.display()
            );
        }
        if name != CHECKSUMS_FILE {
            found.push(name.to_owned());
        }
    }

    let missing: Vec<&str> = expected
        .iter()
        .filter(|name| !found.contains(name))
        .map(String::as_str)
        .collect();
    let mut unexpected: Vec<&str> = found
        .iter()
        .filter(|name| !expected.contains(name))
        .map(String::as_str)
        .collect();
    unexpected.sort();
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("missing {}", missing.join(", ")));
    }
    if !unexpected.is_empty() {
        problems.push(format!("not release assets: {}", unexpected.join(", ")));
    }
    if !problems.is_empty() {
        bail!(
            "{} does not hold exactly the release assets: {}",
            folder.display(),
            problems.join("; ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "2.0.7";

    /// A folder holding every asset of `VERSION`, each file's text being its own name.
    fn release_folder() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        for name in expected_release_assets(VERSION) {
            fs::write(folder.path().join(&name), &name).unwrap();
        }
        folder
    }

    #[test]
    fn checksums_list_every_asset_sorted() {
        let folder = release_folder();
        fs::write(folder.path().join(CHECKSUMS_FILE), "stale").unwrap();

        let written = write_checksums(VERSION, folder.path()).unwrap();

        let text = fs::read_to_string(written).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let expected = expected_release_assets(VERSION);
        assert_eq!(lines.len(), expected.len());
        for (line, name) in lines.iter().zip(&expected) {
            let (hash, listed_name) = line.split_once("  ").unwrap();
            assert_eq!(listed_name, name);
            assert_eq!(hash.len(), 64);
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{line}");
            assert_eq!(hash, sha256_hex(name.as_bytes()));
        }
        let mut sorted = lines.clone();
        sorted.sort_by_key(|line| line.split_once("  ").unwrap().1);
        assert_eq!(lines, sorted);
    }

    #[test]
    fn checksums_refuse_a_missing_asset() {
        let folder = release_folder();
        let gone = "AuroraAssetEditor-2.0.7-macos-universal.dmg";
        fs::remove_file(folder.path().join(gone)).unwrap();

        let message = write_checksums(VERSION, folder.path())
            .unwrap_err()
            .to_string();

        assert!(message.contains(&format!("missing {gone}")), "{message}");
        assert!(!folder.path().join(CHECKSUMS_FILE).exists());
    }

    #[test]
    fn checksums_refuse_an_unexpected_file() {
        let folder = release_folder();
        fs::write(folder.path().join("notes.md"), "notes").unwrap();

        let message = write_checksums(VERSION, folder.path())
            .unwrap_err()
            .to_string();

        assert!(message.contains("notes.md"), "{message}");
    }

    #[test]
    fn checksums_refuse_a_subfolder() {
        let folder = release_folder();
        fs::create_dir(folder.path().join("staging")).unwrap();

        let message = write_checksums(VERSION, folder.path())
            .unwrap_err()
            .to_string();

        assert!(message.contains("`staging`"), "{message}");
    }

    #[test]
    fn assets_of_another_version_are_refused() {
        let folder = release_folder();

        let message = write_checksums("2.0.8", folder.path())
            .unwrap_err()
            .to_string();

        assert!(
            message.contains("AuroraAssetEditor-2.0.8-windows-x86_64.exe"),
            "{message}"
        );
        assert!(
            message.contains("AuroraAssetEditor-2.0.7-windows-x86_64.exe"),
            "{message}"
        );
    }
}
