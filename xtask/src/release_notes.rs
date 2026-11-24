//! `cargo xtask release-notes`: the body of a draft release, composed from GitHub's generated
//! notes, the new features and fixes among the commits, the copy files under
//! `.github/release-notes` and the checksum list.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::notices::repository_root;
use crate::package::FIRST_LAUNCH_SOURCE;

pub const USAGE_LINE: &str = "cargo xtask release-notes --commits <file> --checksums <file> --out <file> [--previous-tag <tag> --generated <file>]";

const FIRST_RELEASE_SOURCE: &str = ".github/release-notes/first-release.md";
const KNOWN_LIMITS_SOURCE: &str = ".github/release-notes/known-limits.md";

const NEW_HEADING: &str = "## New";
const FIXES_HEADING: &str = "## Fixes";
/// Characters Markdown can act on inside a line of text, so a subject keeps every one as typed.
const MARKDOWN_SPECIAL: &[char] = &['\\', '`', '*', '_', '[', ']', '<', '>', '~', '&'];
const KNOWN_LIMITS_HEADING: &str = "## Known limits";
const FIRST_LAUNCH_HEADING: &str = "## macOS first launch";
const CHECKSUMS_HEADING: &str = "## SHA-256 checksums";
const TERMINAL_COMMAND_PREFIX: &str = "xattr ";

/// Everything one release's notes are made of.
pub struct NotesParts {
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

/// The notes in release order. A first release opens with the first-release text and lists no
/// commits; a later one opens with the generated notes, then the `feat` and `fix` commits since the
/// previous tag under "New" and "Fixes". Every release ends with the known limits, the macOS
/// first-launch steps and the checksums.
pub fn compose_notes(parts: &NotesParts) -> Result<String> {
    let mut sections = Vec::new();
    if parts.previous_tag.is_none() {
        sections.push(parts.first_release.trim_end().to_owned());
    } else {
        if let Some(generated) = &parts.generated {
            sections.push(generated.trim_end().to_owned());
        }
        sections.extend(change_sections(&parts.commits));
    }
    sections.push(format!(
        "{KNOWN_LIMITS_HEADING}\n\n{}",
        parts.known_limits.trim_end()
    ));
    sections.push(format!(
        "{FIRST_LAUNCH_HEADING}\n\n{}",
        first_launch_markdown(&parts.macos_first_launch)?
    ));
    sections.push(format!(
        "{CHECKSUMS_HEADING}\n\n```text\n{}\n```",
        parts.checksums.trim_end()
    ));
    Ok(sections.join("\n\n") + "\n")
}

/// "New" with the `feat` commits and "Fixes" with the `fix` commits among the `%h %s` lines, one
/// `- <description> (<short hash>)` line each in `git log` order; a heading without commits is left
/// out, as is every other commit.
fn change_sections(commits: &[String]) -> Vec<String> {
    let mut new = Vec::new();
    let mut fixes = Vec::new();
    for line in commits {
        let (hash, subject) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let Some((kind, description)) = conventional_subject(subject) else {
            continue;
        };
        let item = format!("- {} ({hash})", plain_markdown(description));
        match kind {
            "feat" => new.push(item),
            "fix" => fixes.push(item),
            _ => {}
        }
    }
    [(NEW_HEADING, new), (FIXES_HEADING, fixes)]
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(heading, items)| format!("{heading}\n\n{}", items.join("\n")))
        .collect()
}

/// The type and description of a `type(scope)!: description` subject; the scope and `!` are
/// optional. `None` for a subject in any other form.
fn conventional_subject(subject: &str) -> Option<(&str, &str)> {
    let (head, description) = subject.split_once(": ")?;
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) => {
            let scope = scope.strip_suffix(')')?;
            if scope.is_empty() || scope.contains(['(', ')']) {
                return None;
            }
            kind
        }
        None => head,
    };
    let description = description.trim();
    let is_kind = !kind.is_empty() && kind.chars().all(|character| character.is_ascii_lowercase());
    (is_kind && !description.is_empty()).then_some((kind, description))
}

/// `text` as Markdown that shows it as typed: Markdown's special characters are escaped, and each
/// `@name` is a code span, since GitHub turns an escaped `@name` into a mention as well. A `#123`
/// stays a link to that issue.
fn plain_markdown(text: &str) -> String {
    let mut markdown = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        if character == '@' {
            let name_length = rest[1..]
                .find(|character: char| !is_mention_character(character))
                .unwrap_or(rest.len() - 1);
            if name_length > 0 {
                markdown.push_str(&code_span(&rest[..=name_length]));
                rest = &rest[name_length + 1..];
                continue;
            }
        }
        if MARKDOWN_SPECIAL.contains(&character) {
            markdown.push('\\');
        }
        markdown.push(character);
        rest = &rest[character.len_utf8()..];
    }
    markdown
}

/// A character GitHub reads as part of an `@user` or `@org/team` mention.
fn is_mention_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '/')
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

#[derive(Debug, PartialEq, Eq)]
struct NotesOptions {
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
    let copy = CopyTexts::read(&repository_root())?;
    let (previous_tag, generated) = match options.previous {
        Some(previous) => (
            Some(previous.tag),
            Some(read_filled(&previous.generated, "the generated notes")?),
        ),
        None => (None, None),
    };
    let parts = NotesParts {
        previous_tag,
        generated,
        commits: read_commits(&options.commits)?,
        checksums: read_filled(&options.checksums, "the checksum list")?,
        first_release: copy.first_release,
        known_limits: copy.known_limits,
        macos_first_launch: copy.macos_first_launch,
    };
    std::fs::write(&options.out, compose_notes(&parts)?)
        .with_context(|| format!("could not write {}", options.out.display()))?;
    println!("Wrote {}", options.out.display());
    Ok(())
}

/// The texts every release's notes take from the repository.
struct CopyTexts {
    first_release: String,
    known_limits: String,
    macos_first_launch: String,
}

impl CopyTexts {
    fn read(root: &Path) -> Result<Self> {
        Ok(Self {
            first_release: read_filled(&root.join(FIRST_RELEASE_SOURCE), "the first-release text")?,
            known_limits: read_filled(&root.join(KNOWN_LIMITS_SOURCE), "the known limits")?,
            macos_first_launch: read_filled(
                &root.join(FIRST_LAUNCH_SOURCE),
                "the macOS first-launch text",
            )?,
        })
    }
}

fn parse_options(arguments: &[&str]) -> Result<NotesOptions> {
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
        commits: PathBuf::from(required(commits, "--commits")?),
        checksums: PathBuf::from(required(checksums, "--checksums")?),
        out: PathBuf::from(required(out, "--out")?),
        previous,
    })
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

    fn first_release_parts() -> NotesParts {
        NotesParts {
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
        assert!(!notes.contains(NEW_HEADING), "{notes}");
        assert!(!notes.contains(FIXES_HEADING), "{notes}");
        assert!(!notes.contains("(abc1234)"), "{notes}");
        assert!(position(&notes, "## Changes from 1.x") < position(&notes, KNOWN_LIMITS_HEADING));
        assert!(position(&notes, KNOWN_LIMITS_HEADING) < position(&notes, FIRST_LAUNCH_HEADING));
        assert!(notes.contains("## Known limits\n\n- A limit\n"), "{notes}");
    }

    #[test]
    fn later_release_keeps_generated_notes_then_new_then_fixes() {
        let notes = compose_notes(&later_release_parts()).unwrap();
        assert!(notes.starts_with(GENERATED.trim_end()), "{notes}");
        assert!(!notes.contains("First release text."), "{notes}");
        let ordered = [
            "**Full Changelog**: https://github.com/o/r/compare/v2.0.6...v2.0.7",
            "## New\n\n- w (def5678)\n\n",
            "## Fixes\n\n- y (abc1234)\n\n",
            KNOWN_LIMITS_HEADING,
            FIRST_LAUNCH_HEADING,
            CHECKSUMS_HEADING,
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
    fn release_without_features_or_fixes_leaves_out_both_headings() {
        for commits in [
            Vec::new(),
            vec!["abc1234 test(x): y".to_owned(), "def5678 ci: z".to_owned()],
        ] {
            let parts = NotesParts {
                commits,
                ..later_release_parts()
            };
            let notes = compose_notes(&parts).unwrap();
            assert!(!notes.contains(NEW_HEADING), "{notes}");
            assert!(!notes.contains(FIXES_HEADING), "{notes}");
            assert!(notes.contains(KNOWN_LIMITS_HEADING), "{notes}");
            assert!(notes.contains(FIRST_LAUNCH_HEADING), "{notes}");
        }
    }

    #[test]
    fn only_features_and_fixes_are_listed_without_their_prefix() {
        let parts = NotesParts {
            commits: vec![
                "aaa0001 fix(image): resize banners".to_owned(),
                "aaa0002 test(golden): keep the placeholder".to_owned(),
                "aaa0003 feat!: drop the old format".to_owned(),
                "aaa0004 fix: plain fix".to_owned(),
                "aaa0005 refactor(ui): move things".to_owned(),
                "aaa0006 Merge branch 'other'".to_owned(),
                "aaa0007 feat(editor): lead captions".to_owned(),
                "aaa0008 fix(): empty scope".to_owned(),
                "aaa0009 Fix: capitalised type".to_owned(),
                "aaa0010 fix:no space".to_owned(),
                "aaa0011 ".to_owned(),
            ],
            ..later_release_parts()
        };
        let notes = compose_notes(&parts).unwrap();
        let expected = "## New\n\n\
                        - drop the old format (aaa0003)\n\
                        - lead captions (aaa0007)\n\n\
                        ## Fixes\n\n\
                        - resize banners (aaa0001)\n\
                        - plain fix (aaa0004)\n\n\
                        ## Known limits";
        assert!(notes.contains(expected), "{notes}");
    }

    #[test]
    fn descriptions_show_as_typed_without_mentions() {
        let cases = [
            ("thanks @someone for #123", "thanks `@someone` for #123"),
            ("bump @types/node.", "bump `@types/node`."),
            ("a lone @ sign", "a lone @ sign"),
            (
                "title \"v<version> (<sha>)\"",
                "title \"v\\<version\\> (\\<sha\\>)\"",
            ),
            (
                "quote `code` and *stars* [x]",
                "quote \\`code\\` and \\*stars\\* \\[x\\]",
            ),
            ("snake_case & a\\b ~x~", "snake\\_case \\& a\\\\b \\~x\\~"),
            ("åäö @é", "åäö @é"),
        ];
        for (text, expected) in cases {
            assert_eq!(plain_markdown(text), expected, "{text}");
        }
    }

    #[test]
    fn notes_end_with_first_launch_then_checksums() {
        let notes = compose_notes(&first_release_parts()).unwrap();
        let expected = format!(
            "\n\n- A limit\n\n\
             ## macOS first launch\n\n\
             Open it anyway.\nOr run this in Terminal:\n\n```\nxattr -d q app\n```\n\n\
             ## SHA-256 checksums\n\n```text\n{CHECKSUMS}```\n"
        );
        assert!(notes.ends_with(&expected), "{notes}");
        assert!(!notes.contains("## Build"), "{notes}");
        assert!(!notes.contains("Version:"), "{notes}");
    }

    #[test]
    fn macos_first_launch_is_in_every_release() {
        for parts in [first_release_parts(), later_release_parts()] {
            let notes = compose_notes(&parts).unwrap();
            let first_launch = &notes[position(&notes, FIRST_LAUNCH_HEADING)..];
            assert!(first_launch.contains("Open it anyway.\n"), "{first_launch}");
            assert!(first_launch.contains("xattr -d q app"), "{first_launch}");
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
    fn composes_from_the_repository_copy_files() {
        let copy = CopyTexts::read(&repository_root()).unwrap();
        let parts = NotesParts {
            first_release: copy.first_release.clone(),
            known_limits: copy.known_limits.clone(),
            macos_first_launch: copy.macos_first_launch,
            ..first_release_parts()
        };
        let notes = compose_notes(&parts).unwrap();
        assert!(notes.starts_with(copy.first_release.trim_end()), "{notes}");
        assert!(
            notes.contains(&format!(
                "{KNOWN_LIMITS_HEADING}\n\n{}",
                copy.known_limits.trim_end()
            )),
            "{notes}"
        );
        assert!(notes.contains("```\nxattr "), "{notes}");
    }

    #[test]
    fn read_commits_refuses_a_malformed_line() {
        let folder = tempfile::TempDir::new().unwrap();
        let path = folder.path().join("commits.txt");
        std::fs::write(&path, "abc1234 fix(x): y\ndef5678\n").unwrap();
        let message = read_commits(&path).unwrap_err().to_string();
        assert!(message.contains("`def5678`"), "{message}");

        std::fs::write(&path, "abc1234 fix(x): y\ndef5678 feat(z): w\n").unwrap();
        assert_eq!(
            read_commits(&path).unwrap(),
            ["abc1234 fix(x): y", "def5678 feat(z): w"]
        );
        std::fs::write(&path, "").unwrap();
        assert!(read_commits(&path).unwrap().is_empty());
    }

    #[test]
    fn repeated_option_is_refused() {
        let message = parse_options(&arguments(&["--out", "other.md"]))
            .unwrap_err()
            .to_string();
        assert!(message.contains("--out is given twice"), "{message}");
    }
}
