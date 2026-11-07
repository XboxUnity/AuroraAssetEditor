//! Drives the original `AuroraAsset.dll` from the command line to generate golden fixtures.
//!
//! Exit codes: 0 success, 2 usage or IO error, 4 encode refused, 5 decode refused,
//! 6 DDS decode refused.

#![deny(unsafe_code)]

#[cfg(windows)]
mod cli;
#[cfg(windows)]
mod dll;

#[cfg(windows)]
fn main() {
    std::process::exit(cli::run(std::env::args().skip(1).collect()));
}

#[cfg(not(windows))]
fn main() {
    eprintln!("golden-harness drives a Windows DLL and only runs on Windows");
    std::process::exit(2);
}
