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
    let about = Command::new("cargo")
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
    for line in String::from_utf8_lossy(&about.stderr).lines() {
        if !is_expected_warning(line) {
            eprintln!("{line}");
        }
    }
    if !about.status.success() {
        bail!("`cargo about generate` failed with {}", about.status);
    }
    let generated = fs::read_to_string(output)
        .with_context(|| format!("could not read {}", output.display()))?;
    let members = workspace_member_names(root)?;
    fs::write(output, without_workspace_crates(&generated, &members))
        .with_context(|| format!("could not write {}", output.display()))
}

/// The names of the workspace's own crates. They are the app itself, not third-party software, so the notices
/// leave them out; that also keeps the file the same whatever version a release stamps on the app.
fn workspace_member_names(root: &Path) -> Result<Vec<String>> {
    let metadata = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .context("could not run `cargo metadata`")?;
    if !metadata.status.success() {
        bail!("`cargo metadata` failed with {}", metadata.status);
    }
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout)
        .context("`cargo metadata` printed invalid JSON")?;
    let packages = metadata["packages"]
        .as_array()
        .context("`cargo metadata` listed no packages")?;
    packages
        .iter()
        .map(|package| {
            package["name"]
                .as_str()
                .map(str::to_string)
                .context("a workspace package has no name")
        })
        .collect()
}

/// Drops the workspace members from the generated text: their `<name> <version>` lines, the licence sections
/// left with no user, and their share of the crate counts at the top.
fn without_workspace_crates(generated: &str, members: &[String]) -> String {
    let lines: Vec<&str> = generated.split_inclusive('\n').collect();
    let is_member_entry = |line: &str| {
        members.iter().any(|name| {
            line.strip_prefix("  ")
                .and_then(|rest| rest.strip_prefix(name.as_str()))
                .and_then(|rest| rest.strip_prefix(' '))
                .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit()))
        })
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut removed_per_licence: Vec<(&str, usize)> = Vec::new();
    let mut header_end = None;
    let mut index = 0;
    while index < lines.len() {
        if !starts_licence_section(&lines, index) {
            kept.push(lines[index]);
            index += 1;
            continue;
        }
        header_end.get_or_insert(kept.len());
        let section_end = (index + 1..lines.len())
            .find(|&next| starts_licence_section(&lines, next))
            .unwrap_or(lines.len());
        let user_lines = index + 4..section_end;
        let users = lines[user_lines]
            .iter()
            .take_while(|line| line.starts_with("  "))
            .count();
        let users = index + 4..index + 4 + users;
        let member_users = lines[users.clone()]
            .iter()
            .filter(|line| is_member_entry(line))
            .count();
        if member_users > 0 {
            removed_per_licence.push((lines[index + 1].trim_end(), member_users));
        }
        if member_users != users.len() {
            for (position, line) in lines[index..section_end].iter().enumerate() {
                let is_member_user = users.contains(&(index + position)) && is_member_entry(line);
                if !is_member_user {
                    kept.push(line);
                }
            }
        }
        index = section_end;
    }
    let header_end = header_end.unwrap_or(kept.len());
    let mut result = String::with_capacity(generated.len());
    for (position, line) in kept.iter().enumerate() {
        let reduced = reduced_count_line(line, &removed_per_licence);
        match reduced {
            Some(count_line) if position < header_end => result.push_str(&count_line),
            _ => result.push_str(line),
        }
    }
    result
}

/// A licence section starts with a line of dashes, then the licence name, a blank line and `Used by:`.
fn starts_licence_section(lines: &[&str], index: usize) -> bool {
    index + 3 < lines.len()
        && lines[index].trim_end().len() >= 40
        && lines[index].trim_end().bytes().all(|byte| byte == b'-')
        && lines[index + 2].trim_end().is_empty()
        && lines[index + 3].trim_end() == "Used by:"
}

/// The overview line `<licence> (<n> crates)` with the removed crates subtracted: `None` when the line is not
/// a count line of an affected licence, and an empty text when no crate is left.
fn reduced_count_line(line: &str, removed: &[(&str, usize)]) -> Option<String> {
    let content = line.trim_end();
    let (name, count) = content.strip_suffix(" crates)")?.rsplit_once(" (")?;
    let count: usize = count.parse().ok()?;
    let (_, gone) = removed.iter().find(|(licence, _)| *licence == name)?;
    let left = count.saturating_sub(*gone);
    if left == 0 {
        return Some(String::new());
    }
    Some(format!("{name} ({left} crates){}", &line[content.len()..]))
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

pub(crate) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one folder below the repository root")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(app_version: &str) -> String {
        let dashes = "-".repeat(80);
        format!(
            "Third-party software\n\nMIT License (2 crates)\nThe Unlicense (2 crates)\n\n\
             {dashes}\nMIT License\n\nUsed by:\n  aurora-asset 0.1.0\n  serde 1.0.0 (https://x)\n\nMIT text\n\n\
             {dashes}\nThe Unlicense\n\nUsed by:\n  aurora-asset 0.1.0\n  aurora-asset-editor {app_version}\n\nUnlicense text\n"
        )
    }

    fn members() -> Vec<String> {
        vec![
            "aurora-asset".to_string(),
            "aurora-asset-editor".to_string(),
        ]
    }

    #[test]
    fn stamped_version_does_not_change_the_notices() {
        let before = without_workspace_crates(&generated("2.0.0"), &members());
        let after = without_workspace_crates(&generated("2.0.1"), &members());
        assert_eq!(before, after);
        assert!(!before.contains("aurora-asset"));
    }

    #[test]
    fn only_the_workspace_crates_are_removed() {
        let result = without_workspace_crates(&generated("2.0.0"), &members());
        assert!(result.contains("MIT License (1 crates)\n"));
        assert!(result.contains("  serde 1.0.0 (https://x)\n"));
        assert!(result.contains("MIT text"));
        assert!(!result.contains("Unlicense"));
    }
}
