use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::notices::repository_root;

pub const USAGE_LINE: &str = "cargo xtask set-version <major.minor.patch>";

/// Stamps `version` into the workspace manifest and into the lockfile entries of the crates that inherit it,
/// so that `cargo ... --locked` keeps working afterwards.
pub fn run(version: &str) -> Result<()> {
    validate_version(version)?;
    let root = repository_root();
    let members = members_inheriting_version(&root)?;

    let manifest_path = root.join("Cargo.toml");
    let manifest = set_workspace_version(&read(&manifest_path)?, version)?;
    let lock_path = root.join("Cargo.lock");
    let lock = set_lock_versions(&read(&lock_path)?, &members, version)?;
    write(&manifest_path, &manifest)?;
    write(&lock_path, &lock)?;

    let status = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest_path)
        .stdout(Stdio::null())
        .status()
        .context("could not run `cargo metadata`")?;
    if !status.success() {
        bail!("Cargo.lock is out of sync after the version stamp");
    }
    println!("stamped version {version} on {}", members.join(", "));
    Ok(())
}

fn validate_version(text: &str) -> Result<()> {
    let parts: Vec<&str> = text.split('.').collect();
    let is_number =
        |part: &&str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if parts.len() != 3 || !parts.iter().all(is_number) {
        bail!("`{text}` is not a version of the form major.minor.patch");
    }
    Ok(())
}

/// Replaces the `version = "..."` line of `[workspace.package]`, and nothing else, keeping its line ending.
fn set_workspace_version(manifest: &str, version: &str) -> Result<String> {
    let mut in_workspace_package = false;
    let mut replaced = false;
    let mut result = String::with_capacity(manifest.len());
    for line in manifest.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
        }
        if in_workspace_package && !replaced && is_version_line(trimmed) {
            result.push_str(&format!("version = \"{version}\""));
            result.push_str(&line[content.len()..]);
            replaced = true;
        } else {
            result.push_str(line);
        }
    }
    if !replaced {
        bail!("Cargo.toml has no `version` line in [workspace.package]");
    }
    Ok(result)
}

fn is_version_line(trimmed_line: &str) -> bool {
    trimmed_line
        .strip_prefix("version")
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

/// The names of the workspace members whose `package.version` is `{ workspace = true }`, sorted.
fn members_inheriting_version(root: &Path) -> Result<Vec<String>> {
    let workspace = read_table(&root.join("Cargo.toml"))?;
    let members = workspace
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .context("Cargo.toml has no workspace.members")?;
    let mut names = Vec::new();
    for member in members {
        let folder = member.as_str().context("a workspace member is not text")?;
        let manifest = read_table(&root.join(folder).join("Cargo.toml"))?;
        let package = manifest
            .get("package")
            .with_context(|| format!("{folder}/Cargo.toml has no [package]"))?;
        let inherits = package
            .get("version")
            .and_then(|version| version.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        if inherits {
            let name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .with_context(|| format!("{folder}/Cargo.toml has no package name"))?;
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Sets the version of each named path package (a `[[package]]` entry without a `source` line).
fn set_lock_versions(lock: &str, packages: &[String], version: &str) -> Result<String> {
    let lines: Vec<&str> = lock.split_inclusive('\n').collect();
    let mut replacements: Vec<usize> = Vec::new();
    for name in packages {
        let version_line = path_package_version_line(&lines, name)
            .with_context(|| format!("Cargo.lock has no path package `{name}`"))?;
        replacements.push(version_line);
    }
    let mut result = String::with_capacity(lock.len());
    for (index, line) in lines.iter().enumerate() {
        if replacements.contains(&index) {
            let content = line.trim_end_matches(['\r', '\n']);
            result.push_str(&format!("version = \"{version}\""));
            result.push_str(&line[content.len()..]);
        } else {
            result.push_str(line);
        }
    }
    Ok(result)
}

/// The index of the `version` line in the `[[package]]` entry called `name` that has no `source`.
fn path_package_version_line(lines: &[&str], name: &str) -> Option<usize> {
    let name_line = format!("name = \"{name}\"");
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].trim_end() == "[[package]]")
        .collect();
    starts.iter().enumerate().find_map(|(position, &start)| {
        let end = starts.get(position + 1).copied().unwrap_or(lines.len());
        let entry = &lines[start..end];
        let is_named = entry.iter().any(|line| line.trim_end() == name_line);
        let has_source = entry.iter().any(|line| line.starts_with("source = "));
        if !is_named || has_source {
            return None;
        }
        entry
            .iter()
            .position(|line| line.starts_with("version = "))
            .map(|offset| start + offset)
    })
}

fn read_table(path: &Path) -> Result<toml::Table> {
    read(path)?
        .parse()
        .with_context(|| format!("{} is not valid TOML", path.display()))
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text).with_context(|| format!("could not write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "[workspace]\nmembers = [\"a\"]\n\n[workspace.package]\nversion = \"2.0.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nx = { version = \"1\" }\n";

    #[test]
    fn version_must_be_three_numbers() {
        assert!(validate_version("2.0.12").is_ok());
        for text in ["2.0", "v2.0.1", "2.0.1-rc", "2.0.x", "2..1", ""] {
            let error = validate_version(text).unwrap_err().to_string();
            assert!(error.contains(&format!("`{text}`")), "{error}");
        }
    }

    #[test]
    fn workspace_version_is_replaced_only_in_workspace_package() {
        let stamped = set_workspace_version(MANIFEST, "2.0.7").unwrap();
        assert_eq!(stamped, MANIFEST.replace("\"2.0.0\"", "\"2.0.7\""));
        assert!(stamped.contains("x = { version = \"1\" }"));

        let crlf = MANIFEST.replace('\n', "\r\n");
        let stamped = set_workspace_version(&crlf, "2.0.7").unwrap();
        assert_eq!(stamped, crlf.replace("\"2.0.0\"", "\"2.0.7\""));
    }

    #[test]
    fn missing_workspace_version_is_an_error() {
        let without_table = "[workspace]\nmembers = []\n";
        assert!(set_workspace_version(without_table, "2.0.7").is_err());
        let without_line =
            "[workspace.package]\nedition = \"2024\"\n[other]\nversion = \"1.0.0\"\n";
        assert!(set_workspace_version(without_line, "2.0.7").is_err());
    }

    const LOCK: &str = "[[package]]\nname = \"aurora-asset-editor\"\nversion = \"2.0.0\"\ndependencies = [\"x\"]\n\n[[package]]\nname = \"aurora-asset-editor\"\nversion = \"9.9.9\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

    #[test]
    fn lock_entry_of_a_path_package_is_replaced() {
        let packages = ["aurora-asset-editor".to_string()];
        let stamped = set_lock_versions(LOCK, &packages, "2.0.7").unwrap();
        assert!(
            stamped
                .starts_with("[[package]]\nname = \"aurora-asset-editor\"\nversion = \"2.0.7\"\n")
        );
    }

    #[test]
    fn registry_package_with_the_same_name_is_left() {
        let packages = ["aurora-asset-editor".to_string()];
        let stamped = set_lock_versions(LOCK, &packages, "2.0.7").unwrap();
        assert!(stamped.contains("version = \"9.9.9\"\nsource = "));
        assert_eq!(stamped.matches("2.0.7").count(), 1);
    }

    #[test]
    fn unknown_lock_package_is_an_error() {
        let packages = ["no-such-crate".to_string()];
        let error = set_lock_versions(LOCK, &packages, "2.0.7").unwrap_err();
        assert!(error.to_string().contains("no-such-crate"));
    }

    #[test]
    fn members_inheriting_the_workspace_version() {
        let members = members_inheriting_version(&repository_root()).unwrap();
        assert_eq!(members, ["aurora-asset-editor"]);
    }
}
