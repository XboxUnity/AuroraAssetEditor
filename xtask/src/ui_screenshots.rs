use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

// These names and the platform rule mirror `editor-test-support`, which xtask does not depend on.
const BASELINE_VARIABLE: &str = "AURORA_UI_BASELINE";
const LABEL_VARIABLE: &str = "AURORA_UI_LABEL";
const SCREENS_VARIABLE: &str = "AURORA_UI_SCREENS";
const OUTPUT_FOLDER: &str = "target/ui-screenshots";
const REPORT_FILE: &str = "report.txt";

pub(crate) const EDITOR_PACKAGE: &str = "aurora-asset-editor";
pub(crate) const SCREENSHOT_TEST_TARGET: &str = "ui_screenshots";

pub const USAGE_LINE: &str =
    "cargo xtask ui-screenshots [--baseline <folder>] [--label <text>] [--only <text>]";

#[derive(Debug, PartialEq, Eq, Default)]
pub struct ScreenshotOptions {
    pub baseline: Option<PathBuf>,
    pub label: Option<String>,
    pub only: Option<String>,
}

pub fn parse_options(arguments: &[&str]) -> Result<ScreenshotOptions> {
    let mut options = ScreenshotOptions::default();
    let mut remaining = arguments.iter();
    while let Some(&flag) = remaining.next() {
        if !matches!(flag, "--baseline" | "--label" | "--only") {
            bail!("unknown argument `{flag}`\nusage: {USAGE_LINE}");
        }
        let value = match remaining.next() {
            Some(&value) if !value.is_empty() && !value.starts_with("--") => value,
            _ => bail!("{flag} needs a value\nusage: {USAGE_LINE}"),
        };
        let already_given = match flag {
            "--baseline" => options.baseline.replace(PathBuf::from(value)).is_some(),
            "--label" => options.label.replace(value.to_string()).is_some(),
            _ => options.only.replace(value.to_string()).is_some(),
        };
        if already_given {
            bail!("{flag} was given twice\nusage: {USAGE_LINE}");
        }
    }
    Ok(options)
}

/// `cargo test` runs the test binary in the package folder, not where the user typed the command,
/// so a relative baseline is resolved here, against the folder the user typed it in.
fn absolute_baseline(path: &Path, working_folder: &Path) -> PathBuf {
    working_folder.join(path)
}

/// Runs the screenshot suite and prints its report, which names the contact sheet.
pub fn run(arguments: &[&str]) -> Result<()> {
    let options = parse_options(arguments)?;
    let root = repository_root();
    let output = root.join(OUTPUT_FOLDER).join(current_platform());
    let report = output.join(REPORT_FILE);
    let started = SystemTime::now();

    let mut test = Command::new("cargo");
    test.current_dir(&root).args([
        "test",
        "-p",
        EDITOR_PACKAGE,
        "--test",
        SCREENSHOT_TEST_TARGET,
    ]);
    match &options.baseline {
        Some(path) => {
            let working_folder =
                std::env::current_dir().context("could not read the current folder")?;
            test.env(BASELINE_VARIABLE, absolute_baseline(path, &working_folder))
        }
        None => test.env_remove(BASELINE_VARIABLE),
    };
    match &options.label {
        Some(text) => test.env(LABEL_VARIABLE, text),
        None => test.env_remove(LABEL_VARIABLE),
    };
    match &options.only {
        Some(text) => test.env(SCREENS_VARIABLE, text),
        None => test.env_remove(SCREENS_VARIABLE),
    };
    let status = test
        .status()
        .context("could not run `cargo test` for the UI screenshots")?;

    print_report(&report, started);
    if !status.success() {
        bail!("the UI screenshot tests failed: `cargo test` exited with {status}");
    }
    Ok(())
}

/// A run that never starts (a build error) leaves the report of an earlier run in place, so the
/// report is printed only when this run wrote it. The report itself ends with the contact
/// sheet's URL.
fn print_report(report: &Path, started: SystemTime) {
    match written_since(report, started) {
        Ok(true) => match fs::read_to_string(report) {
            Ok(text) => print!("{text}"),
            Err(error) => println!("Report: could not read {}: {error}", report.display()),
        },
        Ok(false) => println!("Report: not written by this run, see the test output above"),
        Err(error) => println!("Report: could not check {}: {error}", report.display()),
    }
}

/// Whether the file was modified at or after `started`; a file that is not there was not.
fn written_since(path: &Path, started: SystemTime) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.modified()? >= started),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn current_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
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
    fn bare_command_compares_with_the_previous_run() {
        assert_eq!(parse_options(&[]).unwrap(), ScreenshotOptions::default());
    }

    #[test]
    fn options_parse_in_any_order() {
        let options = parse_options(&[
            "--only",
            "shell",
            "--label",
            "v2.0.12 (abc1234)",
            "--baseline",
            "b",
        ])
        .unwrap();
        assert_eq!(options.only.as_deref(), Some("shell"));
        assert_eq!(options.label.as_deref(), Some("v2.0.12 (abc1234)"));
        assert_eq!(options.baseline, Some(PathBuf::from("b")));
    }

    #[test]
    fn a_flag_without_its_value_is_refused() {
        for arguments in [&["--baseline"][..], &["--only", ""][..]] {
            let message = parse_options(arguments).unwrap_err().to_string();
            assert!(message.contains(arguments[0]), "{message}");
        }
    }

    #[test]
    fn a_flag_given_twice_is_refused() {
        for flag in ["--baseline", "--label", "--only"] {
            let message = parse_options(&[flag, "a", flag, "b"])
                .unwrap_err()
                .to_string();
            assert!(message.contains(flag), "{message}");
            assert!(message.contains("twice"), "{message}");
        }
    }

    #[test]
    fn a_value_that_looks_like_a_flag_is_refused() {
        let message = parse_options(&["--baseline", "--only"])
            .unwrap_err()
            .to_string();
        assert!(message.contains("--baseline needs a value"), "{message}");
        assert!(message.contains(USAGE_LINE), "{message}");
    }

    #[test]
    fn the_old_modes_are_refused() {
        for mode in ["check", "update"] {
            let message = parse_options(&[mode]).unwrap_err().to_string();
            assert!(message.contains(mode), "{message}");
            assert!(message.contains(USAGE_LINE), "{message}");
        }
    }

    #[test]
    fn a_report_is_this_runs_only_when_written_after_it_started() {
        let folder = std::env::temp_dir().join(format!("xtask-report-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let report = folder.join(REPORT_FILE);
        assert!(!written_since(&report, SystemTime::now()).unwrap());

        fs::write(&report, "report").unwrap();
        let before = SystemTime::now() - std::time::Duration::from_secs(60);
        let after = SystemTime::now() + std::time::Duration::from_secs(60);
        let written_before_start = written_since(&report, after).unwrap();
        let written_after_start = written_since(&report, before).unwrap();
        fs::remove_dir_all(&folder).unwrap();

        assert!(!written_before_start);
        assert!(written_after_start);
    }

    #[test]
    fn baseline_path_is_made_absolute() {
        let work = std::env::temp_dir().join("work");
        assert_eq!(
            absolute_baseline(Path::new("rel/x"), &work),
            work.join("rel/x")
        );
        let absolute = std::env::temp_dir().join("elsewhere");
        assert_eq!(absolute_baseline(&absolute, &work), absolute);
    }
}
