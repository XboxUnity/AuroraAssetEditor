//! `cargo xtask release-notes`: the body of a draft release, composed from GitHub's generated
//! notes, the commit list, the copy files under `.github/release-notes` and the build details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::notices::repository_root;
use crate::package::FIRST_LAUNCH_SOURCE;
use crate::set_version::validate_version;

pub const USAGE_LINE: &str = "cargo xtask release-notes --version <v> --commit <full sha> --short <hash> --repository <owner/name> --run-url <url> --commits <file> --checksums <file> --out <file> [--previous-tag <tag> --generated <file>]";

const FIRST_RELEASE_SOURCE: &str = ".github/release-notes/first-release.md";
const KNOWN_LIMITS_SOURCE: &str = ".github/release-notes/known-limits.md";

const COMMITS_HEADING: &str = "## Commits";
const KNOWN_LIMITS_HEADING: &str = "## Known limits";
const BUILD_HEADING: &str = "## Build";
const MACOS_TESTING_NOTE: &str = "macOS: tested automatically only";
const TERMINAL_COMMAND_PREFIX: &str = "xattr ";

/// Everything one release's notes are made of.
pub struct NotesParts {
    pub version: String,
    pub commit: String,
    /// The release's one short hash, as the version job printed it; never cut from `commit`.
    pub short_hash: String,
    /// `owner/name` on GitHub.
    pub repository: String,
    pub run_url: String,
    /// The newest published v2 tag; `None` for the first release.
    pub previous_tag: Option<String>,
    /// GitHub's generated notes against `previous_tag`.
    pub generated: Option<String>,
    /// `git log --format='%h %s'` lines since `previous_tag`, newest first.
    pub commits: Vec<String>,
    pub checksums: String,
    pub first_release: String,
    pub known_limits: String,
    pub macos_first_launch: String,
}

/// The notes in release order. A first release opens with the first-release text and has no
/// commit list; a later one opens with the generated notes, then the commits since the previous
/// tag. Every release ends with the known limits and the build section.
pub fn compose_notes(parts: &NotesParts) -> Result<String> {
    let mut sections = Vec::new();
    if parts.previous_tag.is_none() {
        sections.push(parts.first_release.trim_end().to_owned());
    } else {
        if let Some(generated) = &parts.generated {
            sections.push(generated.trim_end().to_owned());
        }
        if !parts.commits.is_empty() {
            sections.push(commits_section(&parts.commits));
        }
    }
    sections.push(format!(
        "{KNOWN_LIMITS_HEADING}\n\n{}",
        parts.known_limits.trim_end()
    ));
    sections.push(build_section(parts)?);
    Ok(sections.join("\n\n") + "\n")
}

/// One `- <subject> (<short hash>)` line per `%h %s` commit line. The subject is code, so an
/// `@name` or `#123` in it does not become a mention or a link on GitHub.
fn commits_section(commits: &[String]) -> String {
    let lines: Vec<String> = commits
        .iter()
        .map(|line| {
            let (hash, subject) = line.split_once(' ').unwrap_or((line.as_str(), ""));
            if subject.is_empty() {
                format!("- ({hash})")
            } else {
                format!("- {} ({hash})", code_span(subject))
            }
        })
        .collect();
    format!("{COMMITS_HEADING}\n\n{}", lines.join("\n"))
}

/// `text` as a Markdown code span: fenced by one backtick more than its longest backtick run,
/// with a space inside each fence when it starts or ends with a backtick (CommonMark strips them).
fn code_span(text: &str) -> String {
    let longest_run = text
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest_run + 1);
    if text.starts_with('`') || text.ends_with('`') {
        format!("{fence} {text} {fence}")
    } else {
        format!("{fence}{text}{fence}")
    }
}

fn build_section(parts: &NotesParts) -> Result<String> {
    let commit_url = format!(
        "https://github.com/{}/commit/{}",
        parts.repository, parts.commit
    );
    Ok(format!(
        "{BUILD_HEADING}\n\n\
         - Version: {version} ({short_hash})\n\
         - Commit: [{commit}]({commit_url})\n\
         - Built by: [GitHub Actions run]({run_url})\n\
         - {MACOS_TESTING_NOTE}\n\n\
         ### macOS first launch\n\n\
         {first_launch}\n\n\
         ### SHA-256 checksums\n\n\
         ```text\n{checksums}\n```",
        version = parts.version,
        short_hash = parts.short_hash,
        commit = parts.commit,
        run_url = parts.run_url,
        first_launch = first_launch_markdown(&parts.macos_first_launch)?,
        checksums = parts.checksums.trim_end(),
    ))
}

/// The first-launch text with its Terminal command, the one line starting with `xattr `, in a
/// code block. The text file stays plain because the disk image carries it as it is.
fn first_launch_markdown(text: &str) -> Result<String> {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let commands: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with(TERMINAL_COMMAND_PREFIX))
        .map(|(index, _)| index)
        .collect();
    let [command] = commands[..] else {
        bail!(
            "the macOS first-launch text ({FIRST_LAUNCH_SOURCE}) has {} lines starting with `{TERMINAL_COMMAND_PREFIX}`; the release notes show exactly one as the Terminal command",
            commands.len()
        );
    };
    let mut blocks = Vec::new();
    let before = lines[..command].join("\n");
    if !before.trim().is_empty() {
        blocks.push(before.trim_end().to_owned());
    }
    blocks.push(format!("```\n{}\n```", lines[command]));
    let after = lines[command + 1..].join("\n");
    if !after.trim().is_empty() {
        blocks.push(after.trim().to_owned());
    }
    Ok(blocks.join("\n\n"))
}

/// The `release-notes` command line, checked.
#[derive(Debug, PartialEq, Eq)]
struct NotesOptions {
    version: String,
    commit: String,
    short_hash: String,
    repository: String,
    run_url: String,
    commits: PathBuf,
    checksums: PathBuf,
    out: PathBuf,
    previous: Option<PreviousRelease>,
}

/// The previous v2 release and GitHub's notes generated against it: given together or not at all.
#[derive(Debug, PartialEq, Eq)]
struct PreviousRelease {
    tag: String,
    generated: PathBuf,
}

/// Composes the notes from the command line's files and the repository's copy files, and writes
/// them to `--out`.
pub fn run(arguments: &[&str]) -> Result<()> {
    let options = parse_options(arguments)?;
    let root = repository_root();
    let (previous_tag, generated) = match options.previous {
        Some(previous) => (
            Some(previous.tag),
            Some(read_filled(&previous.generated, "the generated notes")?),
        ),
        None => (None, None),
    };
    let parts = NotesParts {
        version: options.version,
        commit: options.commit,
        short_hash: options.short_hash,
        repository: options.repository,
        run_url: options.run_url,
        previous_tag,
        generated,
        commits: read_commits(&options.commits)?,
        checksums: read_filled(&options.checksums, "the checksum list")?,
        first_release: read_filled(&root.join(FIRST_RELEASE_SOURCE), "the first-release text")?,
        known_limits: read_filled(&root.join(KNOWN_LIMITS_SOURCE), "the known limits")?,
        macos_first_launch: read_filled(
            &root.join(FIRST_LAUNCH_SOURCE),
            "the macOS first-launch text",
        )?,
    };
    std::fs::write(&options.out, compose_notes(&parts)?)
        .with_context(|| format!("could not write {}", options.out.display()))?;
    println!("Wrote {}", options.out.display());
    Ok(())
}

fn parse_options(arguments: &[&str]) -> Result<NotesOptions> {
    let mut version = None;
    let mut commit = None;
    let mut short_hash = None;
    let mut repository = None;
    let mut run_url = None;
    let mut commits = None;
    let mut checksums = None;
    let mut out = None;
    let mut previous_tag = None;
    let mut generated = None;
    let mut remaining = arguments.iter();
    while let Some(&flag) = remaining.next() {
        let Some(&value) = remaining.next() else {
            bail!("{flag} needs a value; usage: {USAGE_LINE}");
        };
        let slot = match flag {
            "--version" => &mut version,
            "--commit" => &mut commit,
            "--short" => &mut short_hash,
            "--repository" => &mut repository,
            "--run-url" => &mut run_url,
            "--commits" => &mut commits,
            "--checksums" => &mut checksums,
            "--out" => &mut out,
            "--previous-tag" => &mut previous_tag,
            "--generated" => &mut generated,
            other => bail!("unknown option `{other}`; usage: {USAGE_LINE}"),
        };
        if slot.replace(value.to_owned()).is_some() {
            bail!("{flag} is given twice");
        }
    }

    let required = |value: Option<String>, flag: &str| match value {
        Some(value) => Ok(value),
        None => Err(anyhow::anyhow!("{flag} is required; usage: {USAGE_LINE}")),
    };
    let version = required(version, "--version")?;
    validate_version(&version)?;
    let commit = required(commit, "--commit")?;
    if !is_full_commit(&commit) {
        bail!("--commit `{commit}` is not a full commit hash: 40 lowercase hex digits");
    }
    let short_hash = required(short_hash, "--short")?;
    if short_hash.is_empty() || !commit.starts_with(&short_hash) {
        bail!("--short `{short_hash}` is not the start of --commit `{commit}`");
    }
    let repository = required(repository, "--repository")?;
    if !is_owner_and_name(&repository) {
        bail!("--repository `{repository}` is not of the form owner/name");
    }
    let run_url = required(run_url, "--run-url")?;
    let run_prefix = format!("https://github.com/{repository}/actions/runs/");
    if !run_url.strip_prefix(&run_prefix).is_some_and(is_all_digits) {
        bail!("--run-url `{run_url}` is not a run of the repository: {run_prefix}<digits>");
    }
    let previous = match (previous_tag, generated) {
        (Some(tag), Some(generated)) => Some(PreviousRelease {
            tag,
            generated: PathBuf::from(generated),
        }),
        (None, None) => None,
        (Some(_), None) => {
            bail!("--previous-tag needs --generated, the notes generated against it")
        }
        (None, Some(_)) => {
            bail!("--generated needs --previous-tag, the tag it was generated against")
        }
    };
    Ok(NotesOptions {
        version,
        commit,
        short_hash,
        repository,
        run_url,
        commits: PathBuf::from(required(commits, "--commits")?),
        checksums: PathBuf::from(required(checksums, "--checksums")?),
        out: PathBuf::from(required(out, "--out")?),
        previous,
    })
}

fn is_full_commit(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_owner_and_name(repository: &str) -> bool {
    match repository.split_once('/') {
        Some((owner, name)) => !owner.is_empty() && !name.is_empty() && !name.contains('/'),
        None => false,
    }
}

/// A file the notes need, refused when it holds nothing, since the notes would then miss a part.
fn read_filled(path: &Path, what: &str) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {what} from {}", path.display()))?;
    if text.trim().is_empty() {
        bail!("{} holds no text for {what}", path.display());
    }
    Ok(text)
}

/// The `%h %s` lines of `git log`; the file is empty when nothing was committed since the
/// previous tag.
fn read_commits(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read the commit list from {}", path.display()))?;
    text.lines()
        .map(|line| match line.split_once(' ') {
            Some((hash, _)) if !hash.is_empty() => Ok(line.to_owned()),
            _ => bail!(
                "{}: `{line}` is not a `<short hash> <subject>` line",
                path.display()
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATED: &str = "## What's Changed\n* Nothing from pull requests\n\n**Full Changelog**: https://github.com/o/r/compare/v2.0.6...v2.0.7\n";
    const CHECKSUMS: &str = "0123abcd  AuroraAssetEditor-2.0.7-windows-x86_64.exe\n4567ef01  AuroraAssetEditor-2.0.7-linux-x86_64.AppImage\n";
    const COMMIT: &str = "f00dfeedf00dfeedf00dfeedf00dfeedf00dfeed";

    fn first_release_parts() -> NotesParts {
        NotesParts {
            version: "2.0.7".to_owned(),
            commit: COMMIT.to_owned(),
            short_hash: "a1b2c3d".to_owned(),
            repository: "o/r".to_owned(),
            run_url: "https://github.com/o/r/actions/runs/42".to_owned(),
            previous_tag: None,
            generated: None,
            commits: vec!["abc1234 fix(x): y".to_owned()],
            checksums: CHECKSUMS.to_owned(),
            first_release: "First release text.\n\n## Changes from 1.x\n\n- One\n".to_owned(),
            known_limits: "- A limit\n".to_owned(),
            macos_first_launch: "Open it anyway.\nOr run this in Terminal:\nxattr -d q app\n"
                .to_owned(),
        }
    }

    fn later_release_parts() -> NotesParts {
        NotesParts {
            previous_tag: Some("v2.0.6".to_owned()),
            generated: Some(GENERATED.to_owned()),
            commits: vec![
                "abc1234 fix(x): y".to_owned(),
                "def5678 feat(z): w".to_owned(),
            ],
            ..first_release_parts()
        }
    }

    fn position(notes: &str, text: &str) -> usize {
        notes
            .find(text)
            .unwrap_or_else(|| panic!("{text:?} is missing from:\n{notes}"))
    }

    fn arguments(extra: &[&'static str]) -> Vec<&'static str> {
        let mut all = vec![
            "--version",
            "2.0.7",
            "--commit",
            COMMIT,
            "--short",
            "f00dfee",
            "--repository",
            "o/r",
            "--run-url",
            "https://github.com/o/r/actions/runs/42",
            "--commits",
            "commits.txt",
            "--checksums",
            "SHA256SUMS.txt",
            "--out",
            "notes.md",
        ];
        all.extend_from_slice(extra);
        all
    }

    #[test]
    fn first_release_has_no_compare_link_and_no_commits() {
        let notes = compose_notes(&first_release_parts()).unwrap();
        assert!(notes.starts_with("First release text.\n\n## Changes from 1.x\n\n- One\n"));
        assert!(!notes.contains("compare/"), "{notes}");
        assert!(!notes.contains(COMMITS_HEADING), "{notes}");
        assert!(!notes.contains("fix(x): y"), "{notes}");
        assert!(position(&notes, "## Changes from 1.x") < position(&notes, KNOWN_LIMITS_HEADING));
        assert!(position(&notes, KNOWN_LIMITS_HEADING) < position(&notes, BUILD_HEADING));
        assert!(notes.contains("## Known limits\n\n- A limit\n"), "{notes}");
    }

    #[test]
    fn later_release_keeps_generated_notes_then_commits() {
        let notes = compose_notes(&later_release_parts()).unwrap();
        assert!(notes.starts_with(GENERATED.trim_end()), "{notes}");
        assert!(!notes.contains("First release text."), "{notes}");
        let ordered = [
            "**Full Changelog**: https://github.com/o/r/compare/v2.0.6...v2.0.7",
            COMMITS_HEADING,
            "- `fix(x): y` (abc1234)",
            "- `feat(z): w` (def5678)",
            KNOWN_LIMITS_HEADING,
            BUILD_HEADING,
        ];
        for pair in ordered.windows(2) {
            assert!(
                position(&notes, pair[0]) < position(&notes, pair[1]),
                "{:?} should come before {:?} in:\n{notes}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn empty_commit_list_leaves_out_the_section() {
        let parts = NotesParts {
            commits: Vec::new(),
            ..later_release_parts()
        };
        let notes = compose_notes(&parts).unwrap();
        assert!(!notes.contains(COMMITS_HEADING), "{notes}");
        assert!(notes.contains(KNOWN_LIMITS_HEADING), "{notes}");
        assert!(notes.contains(BUILD_HEADING), "{notes}");
    }

    #[test]
    fn commit_subjects_cannot_mention_or_link() {
        let parts = NotesParts {
            commits: vec![
                "abc1234 fix: thanks @someone for #123".to_owned(),
                "def5678 docs: quote `code` here".to_owned(),
                "aaa1111 `tick` first".to_owned(),
                "bbb2222 ".to_owned(),
            ],
            ..later_release_parts()
        };
        let notes = compose_notes(&parts).unwrap();
        let expected = "## Commits\n\n\
                        - `fix: thanks @someone for #123` (abc1234)\n\
                        - ``docs: quote `code` here`` (def5678)\n\
                        - `` `tick` first `` (aaa1111)\n\
                        - (bbb2222)\n";
        assert!(notes.contains(expected), "{notes}");
    }

    #[test]
    fn build_section_lists_version_commit_run_and_checksums() {
        let notes = compose_notes(&first_release_parts()).unwrap();
        let build = &notes[position(&notes, BUILD_HEADING)..];
        assert!(build.contains("2.0.7 (a1b2c3d)"), "{build}");
        assert!(
            build.contains(&format!("https://github.com/o/r/commit/{COMMIT}")),
            "{build}"
        );
        assert!(
            build.contains("https://github.com/o/r/actions/runs/42"),
            "{build}"
        );
        assert!(
            build.contains(&format!("```text\n{CHECKSUMS}```\n")),
            "{build}"
        );
    }

    #[test]
    fn macos_note_is_in_every_release() {
        for parts in [first_release_parts(), later_release_parts()] {
            let notes = compose_notes(&parts).unwrap();
            let build = &notes[position(&notes, BUILD_HEADING)..];
            assert!(build.contains(MACOS_TESTING_NOTE), "{build}");
            assert!(build.contains("Open it anyway.\n"), "{build}");
            assert!(build.contains("xattr -d q app"), "{build}");
        }
    }

    #[test]
    fn first_launch_command_is_a_code_block() {
        let text = std::fs::read_to_string(repository_root().join(FIRST_LAUNCH_SOURCE)).unwrap();
        let parts = NotesParts {
            macos_first_launch: text,
            ..first_release_parts()
        };
        let notes = compose_notes(&parts).unwrap();
        let expected = "Or run this in Terminal:\n\n\
                        ```\n\
                        xattr -d com.apple.quarantine \"/Applications/Aurora Asset Editor.app\"\n\
                        ```\n";
        assert!(notes.contains(expected), "{notes}");
        assert!(
            notes.contains("Aurora Asset Editor is not signed by Apple"),
            "{notes}"
        );
    }

    #[test]
    fn first_launch_text_without_a_command_is_refused() {
        let parts = NotesParts {
            macos_first_launch: "Open it anyway.\n".to_owned(),
            ..first_release_parts()
        };
        let message = compose_notes(&parts).unwrap_err().to_string();
        assert!(message.contains("`xattr `"), "{message}");

        let parts = NotesParts {
            macos_first_launch: "xattr -d a\nxattr -d b\n".to_owned(),
            ..first_release_parts()
        };
        let message = compose_notes(&parts).unwrap_err().to_string();
        assert!(message.contains("2 lines"), "{message}");
    }

    #[test]
    fn previous_tag_without_generated_notes_is_refused() {
        let message = parse_options(&arguments(&["--previous-tag", "v2.0.6"]))
            .unwrap_err()
            .to_string();
        assert!(message.contains("--generated"), "{message}");
        let message = parse_options(&arguments(&["--generated", "generated.md"]))
            .unwrap_err()
            .to_string();
        assert!(message.contains("--previous-tag"), "{message}");

        let options = parse_options(&arguments(&[
            "--previous-tag",
            "v2.0.6",
            "--generated",
            "generated.md",
        ]))
        .unwrap();
        assert_eq!(
            options.previous,
            Some(PreviousRelease {
                tag: "v2.0.6".to_owned(),
                generated: PathBuf::from("generated.md"),
            })
        );
        assert_eq!(parse_options(&arguments(&[])).unwrap().previous, None);
    }

    #[test]
    fn commit_must_be_40_lowercase_hex_digits() {
        for commit in [
            "F00DFEEDF00DFEEDF00DFEEDF00DFEEDF00DFEED",
            "f00dfeedf00dfeedf00dfeedf00dfeedf00dfee",
            "f00dfeedf00dfeedf00dfeedf00dfeedf00dfeedf",
            "f00dfeedf00dfeedf00dfeedf00dfeedf00dfeeg",
        ] {
            let mut all = arguments(&[]);
            all[3] = commit;
            let message = parse_options(&all).unwrap_err().to_string();
            assert!(message.contains("--commit"), "{message}");
            assert!(message.contains("40 lowercase hex digits"), "{message}");
        }
    }

    #[test]
    fn run_url_must_be_a_run_of_the_repository() {
        for run_url in [
            "https://github.com/o/r/actions/runs/",
            "https://github.com/o/r/actions/runs/4x2",
            "https://github.com/o/r/actions/runs/42/attempts/1",
            "https://github.com/other/r/actions/runs/42",
            "http://github.com/o/r/actions/runs/42",
        ] {
            let mut all = arguments(&[]);
            all[9] = run_url;
            let message = parse_options(&all).unwrap_err().to_string();
            assert!(message.contains("--run-url"), "{message}");
            assert!(
                message.contains("https://github.com/o/r/actions/runs/<digits>"),
                "{message}"
            );
        }
    }

    #[test]
    fn short_hash_must_start_the_commit() {
        let mut all = arguments(&[]);
        all[5] = "a1b2c3d";
        let message = parse_options(&all).unwrap_err().to_string();
        assert!(message.contains("a1b2c3d"), "{message}");
    }
}
