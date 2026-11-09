use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::asset_names::expected_release_assets;
use crate::golden::sha256_hex;
use crate::set_version::validate_version;

pub const USAGE_LINE: &str = "cargo xtask checksums <version> <folder> --out <file>";

/// Checks that `folder` holds exactly the release assets of `version` and writes their checksum
/// list to `out`, which the release notes show. The list is not a release asset itself, so it
/// is written outside the folder.
pub fn run(version: &str, folder: &str, out: &str) -> Result<()> {
    validate_version(version)?;
    write_checksums(version, Path::new(folder), Path::new(out))?;
    println!("Wrote {out}");
    Ok(())
}

/// Writes `<sha256 hex>  <name>` per asset, sorted by name: the `sha256sum` format, so
/// `sha256sum -c` checks a download.
pub fn write_checksums(version: &str, folder: &Path, out: &Path) -> Result<()> {
    check_outside(folder, out)?;
    let expected = expected_release_assets(version);
    check_folder_holds_exactly(folder, &expected)?;
    let mut text = String::new();
    for name in &expected {
        let path = folder.join(name);
        let bytes =
            fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
        text.push_str(&format!("{}  {name}\n", sha256_hex(&bytes)));
    }
    fs::write(out, text).with_context(|| format!("could not write {}", out.display()))
}

/// Everything in the release folder is uploaded, so the list must be written elsewhere.
fn check_outside(folder: &Path, out: &Path) -> Result<()> {
    let out_folder = match out.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let resolved = |path: &Path| {
        path.canonicalize()
            .with_context(|| format!("could not resolve {}", path.display()))
    };
    if resolved(out_folder)? == resolved(folder)? {
        bail!(
            "the checksum list {} must be written outside the release folder {}, whose files are all uploaded",
            out.display(),
            folder.display()
        );
    }
    Ok(())
}

/// The folder's files must be the expected assets; a subfolder is refused too, since a release
/// attaches only files.
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
        found.push(name.to_owned());
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
    use std::path::PathBuf;

    const VERSION: &str = "2.0.7";

    /// What the test writes into the asset `name`: not the name itself, so that hashing the name instead of
    /// the file fails the test.
    fn contents_of(name: &str) -> String {
        format!("contents of {name}")
    }

    /// A folder holding every asset of `VERSION`.
    fn release_folder() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        for name in expected_release_assets(VERSION) {
            fs::write(folder.path().join(&name), contents_of(&name)).unwrap();
        }
        folder
    }

    /// Where the tests write the checksum list: beside the release folder, never in it.
    fn list_path(outside: &tempfile::TempDir) -> PathBuf {
        outside.path().join("checksums.txt")
    }

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn checksums_list_every_asset_sorted() {
        let folder = release_folder();
        let outside = tempfile::tempdir().unwrap();

        write_checksums(VERSION, folder.path(), &list_path(&outside)).unwrap();

        let text = fs::read_to_string(list_path(&outside)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let expected = expected_release_assets(VERSION);
        assert_eq!(lines.len(), expected.len());
        for (line, name) in lines.iter().zip(&expected) {
            let (hash, listed_name) = line.split_once("  ").unwrap();
            assert_eq!(listed_name, name);
            assert_eq!(hash.len(), 64);
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{line}");
            assert_eq!(hash, sha256_hex(contents_of(name).as_bytes()));
        }
        let mut sorted = lines.clone();
        sorted.sort_by_key(|line| line.split_once("  ").unwrap().1);
        assert_eq!(lines, sorted);
        assert_eq!(file_names(folder.path()), expected);
    }

    #[test]
    fn checksum_list_inside_the_release_folder_is_refused() {
        let folder = release_folder();
        let inside = folder.path().join("SHA256SUMS.txt");

        let message = write_checksums(VERSION, folder.path(), &inside)
            .unwrap_err()
            .to_string();

        assert!(message.contains("outside"), "{message}");
        assert!(!inside.exists());
    }

    #[test]
    fn checksums_refuse_a_missing_asset() {
        let folder = release_folder();
        let outside = tempfile::tempdir().unwrap();
        let gone = "AuroraAssetEditor-2.0.7-macos-universal.dmg";
        fs::remove_file(folder.path().join(gone)).unwrap();

        let message = write_checksums(VERSION, folder.path(), &list_path(&outside))
            .unwrap_err()
            .to_string();

        assert!(message.contains(&format!("missing {gone}")), "{message}");
        assert!(!list_path(&outside).exists());
    }

    #[test]
    fn checksums_refuse_an_unexpected_file() {
        let folder = release_folder();
        let outside = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("notes.md"), "notes").unwrap();
        // A checksum list is not a release asset either.
        fs::write(folder.path().join("SHA256SUMS.txt"), "stale").unwrap();

        let message = write_checksums(VERSION, folder.path(), &list_path(&outside))
            .unwrap_err()
            .to_string();

        assert!(
            message.contains("not release assets: SHA256SUMS.txt, notes.md"),
            "{message}"
        );
    }

    #[test]
    fn checksums_refuse_a_subfolder() {
        let folder = release_folder();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(folder.path().join("staging")).unwrap();

        let message = write_checksums(VERSION, folder.path(), &list_path(&outside))
            .unwrap_err()
            .to_string();

        assert!(message.contains("`staging`"), "{message}");
    }

    #[test]
    fn assets_of_another_version_are_refused() {
        let folder = release_folder();
        let outside = tempfile::tempdir().unwrap();

        let message = write_checksums("2.0.8", folder.path(), &list_path(&outside))
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
