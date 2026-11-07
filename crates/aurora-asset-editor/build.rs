//! Build information for the About dialog and the build pill.
//!
//! The dirty flag means "the working tree had changes when this script last ran". Cargo reruns the script when
//! the checked-out commit, the index or one of the two environment variables changes, but not when a tracked
//! file is edited without staging it, so a local build can show a stale flag. CI builds are clean.

use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};

const UNKNOWN: &str = "unknown";
const BUILD_DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
const GIT_FOLDER: &str = "../../.git";
const REF_PREFIX: &str = "ref: ";

fn main() {
    watch_git_state();
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=AURORA_BUILD_RUN_URL");

    let full_hash = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| UNKNOWN.to_owned());
    let short_hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| UNKNOWN.to_owned());
    let dirty = git(&["status", "--porcelain"]).is_some_and(|status| !status.is_empty());
    let target = std::env::var("TARGET").unwrap_or_else(|_| UNKNOWN.to_owned());
    let run_url = std::env::var("AURORA_BUILD_RUN_URL").unwrap_or_default();

    println!("cargo:rustc-env=AURORA_GIT_HASH={short_hash}");
    println!("cargo:rustc-env=AURORA_GIT_HASH_FULL={full_hash}");
    println!("cargo:rustc-env=AURORA_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=AURORA_BUILD_DATE={}", build_date());
    println!("cargo:rustc-env=AURORA_TARGET={target}");
    println!("cargo:rustc-env=AURORA_BUILD_RUN_URL={run_url}");
}

/// A commit changes the ref file `HEAD` points to, not `HEAD` itself, and a packed ref lives in `packed-refs`.
fn watch_git_state() {
    println!("cargo:rerun-if-changed={GIT_FOLDER}/HEAD");
    println!("cargo:rerun-if-changed={GIT_FOLDER}/packed-refs");
    println!("cargo:rerun-if-changed={GIT_FOLDER}/index");
    let head = fs::read_to_string(Path::new(GIT_FOLDER).join("HEAD")).unwrap_or_default();
    if let Some(reference) = head.trim().strip_prefix(REF_PREFIX) {
        println!("cargo:rerun-if-changed={GIT_FOLDER}/{reference}");
    }
}

/// The output of `git` run with `arguments`; `None` with a warning when it fails.
fn git(arguments: &[&str]) -> Option<String> {
    let output = match Command::new("git").args(arguments).output() {
        Ok(output) => output,
        Err(error) => {
            println!(
                "cargo:warning=could not run git {}: {error}",
                arguments.join(" ")
            );
            return None;
        }
    };
    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr);
        println!(
            "cargo:warning=git {} failed: {}",
            arguments.join(" "),
            reason.trim()
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// UTC time of the build, or of `SOURCE_DATE_EPOCH` when it is set.
fn build_date() -> String {
    let epoch = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|text| {
        let parsed = text.trim().parse::<i64>().ok();
        if parsed.is_none() {
            println!("cargo:warning=SOURCE_DATE_EPOCH is not a number: {text}");
        }
        parsed
    });
    let moment = epoch
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .unwrap_or_else(Utc::now);
    moment.format(BUILD_DATE_FORMAT).to_string()
}
