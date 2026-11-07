//! Build information for the About dialog and the build pill.
//!
//! The dirty flag means "the working tree had changes when this script last ran". Cargo reruns the script when
//! the checked-out commit, the index or one of the two environment variables changes, but not when a tracked
//! file is edited without staging it, so a local build can show a stale flag. CI builds are clean. Without a
//! `.git` folder (a source archive or a git worktree) the git state is not watched and the script reruns on each
//! build; git itself then fails, which gives `unknown` and a warning.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::write::GzEncoder;

const UNKNOWN: &str = "unknown";
const BUILD_DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
const GIT_FOLDER: &str = "../../.git";
const REF_PREFIX: &str = "ref: ";

const NOTICE_FILES: [&str; 2] = ["notices/handwritten.txt", "notices/third-party.txt"];
const COMPRESSED_NOTICES: &str = "notices.gz";

fn main() {
    watch_git_state();
    pack_notices();
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
    let head_path = Path::new(GIT_FOLDER).join("HEAD");
    match fs::read_to_string(&head_path) {
        Ok(head) => {
            if let Some(reference) = head.trim().strip_prefix(REF_PREFIX) {
                println!("cargo:rerun-if-changed={GIT_FOLDER}/{reference}");
            }
        }
        Err(error) => println!(
            "cargo:warning=could not read {}: {error}; a commit will not rerun the build script",
            head_path.display()
        ),
    }
}

/// The notices as one gzip file in `OUT_DIR`: the binary carries it compressed and unpacks it when the
/// About dialog's notices are opened.
fn pack_notices() {
    let mut text = String::new();
    for file in NOTICE_FILES {
        println!("cargo:rerun-if-changed={file}");
        let part = fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("could not read {file}: {error}"));
        text.push_str(&part);
        text.push('\n');
    }
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let target = Path::new(&out_dir).join(COMPRESSED_NOTICES);
    let file = File::create(&target)
        .unwrap_or_else(|error| panic!("could not create {}: {error}", target.display()));
    let mut encoder = GzEncoder::new(file, Compression::best());
    encoder
        .write_all(text.as_bytes())
        .and_then(|()| encoder.finish().map(|_| ()))
        .unwrap_or_else(|error| panic!("could not write {}: {error}", target.display()));
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
