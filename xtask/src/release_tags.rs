use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const LATEST_TAG_USAGE_LINE: &str = "cargo xtask latest-v2-tag";
pub const NEXT_VERSION_USAGE_LINE: &str = "cargo xtask next-v2-version";

/// A tag of the form `v2.<minor>.<patch>`, with nothing but digits in each number.
fn parse_v2_tag(tag: &str) -> Option<(u64, u64)> {
    let numbers = tag.strip_prefix("v2.")?;
    let (minor, patch) = numbers.split_once('.')?;
    let all_digits =
        |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !all_digits(minor) || !all_digits(patch) {
        return None;
    }
    Some((minor.parse().ok()?, patch.parse().ok()?))
}

/// The highest `v2.<minor>.<patch>` tag by its numbers; every other tag is ignored.
pub fn latest_v2_tag(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter_map(|tag| parse_v2_tag(tag).map(|numbers| (numbers, tag)))
        .max_by_key(|(numbers, _)| *numbers)
        .map(|(_, tag)| tag.clone())
}

/// The latest v2 tag with its patch plus one, or `2.0.0` before the first v2 release.
pub fn next_v2_version(tags: &[String]) -> String {
    match latest_v2_tag(tags).as_deref().and_then(parse_v2_tag) {
        Some((minor, patch)) => format!("2.{minor}.{}", patch + 1),
        None => "2.0.0".to_owned(),
    }
}

pub fn repository_tags(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["tag", "--list", "v2.*"])
        .output()
        .context("could not run `git tag --list`")?;
    if !output.status.success() {
        bail!(
            "`git tag --list v2.*` failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text =
        String::from_utf8(output.stdout).context("`git tag` printed text that is not UTF-8")?;
    Ok(text.lines().map(str::to_owned).collect())
}

pub fn print_latest_tag() -> Result<()> {
    let tags = repository_tags(&crate::ui_screenshots::repository_root())?;
    if let Some(tag) = latest_v2_tag(&tags) {
        println!("{tag}");
    }
    Ok(())
}

pub fn print_next_version() -> Result<()> {
    let tags = repository_tags(&crate::ui_screenshots::repository_root())?;
    println!("{}", next_v2_version(&tags));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn latest_tag_is_the_highest_v2_version() {
        let all = tags(&[
            "v1.4.14",
            "v2.0.9",
            "v2.0.10",
            "v2.1.0-rc",
            "v20.0.0",
            "v2.0.2",
            "2.0.11",
        ]);
        assert_eq!(latest_v2_tag(&all).as_deref(), Some("v2.0.10"));
    }

    #[test]
    fn no_v2_tag_gives_none() {
        assert_eq!(latest_v2_tag(&tags(&["v1.4.11", "v1.4.14"])), None);
    }

    #[test]
    fn next_version_is_the_latest_v2_tag_plus_one() {
        assert_eq!(
            next_v2_version(&tags(&["v1.4.14", "v2.0.9", "v2.0.10"])),
            "2.0.11"
        );
        assert_eq!(next_v2_version(&tags(&["v2.1.3"])), "2.1.4");
    }

    #[test]
    fn next_version_without_v2_tags_is_2_0_0() {
        assert_eq!(next_v2_version(&[]), "2.0.0");
        assert_eq!(next_v2_version(&tags(&["v1.4.11"])), "2.0.0");
    }
}
