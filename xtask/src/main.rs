#![forbid(unsafe_code)]

mod closeness_inputs;
mod content_db;
mod crafted;
mod cross_decode;
mod dds_sample;
mod gdiplus_measurements;
mod gdiplus_report;
mod gdiplus_tool;
mod golden;
mod gray16_table;
mod minimum_read;
mod notices;
mod prng;
mod record_online;
mod srgb_profile;
mod surface_unit;
mod synthetic;
mod ui_screenshots;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE_LINES: [&str; 6] = [
    "cargo xtask golden <generate [--refresh-inputs] | check>",
    "cargo xtask cross-decode [<image files>...]",
    "cargo xtask cross-samples",
    "cargo xtask content-db-samples",
    "cargo xtask notices [--check]",
    ui_screenshots::USAGE_LINE,
];

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = match arguments.as_slice() {
        ["golden", "generate"] => golden::generate(false),
        ["golden", "generate", "--refresh-inputs"] => golden::generate(true),
        ["golden", "check"] => golden::check(),
        ["cross-samples"] => cross_decode::write_samples(),
        ["content-db-samples"] => content_db::write_samples(),
        ["closeness-inputs"] => closeness_inputs::write_gate_inputs(),
        ["gdiplus-report"] => gdiplus_report::write_report(),
        ["gray16-table"] => gray16_table::write_table(),
        ["record-online"] => record_online::record(),
        ["notices"] => notices::run(false),
        ["notices", "--check"] => notices::run(true),
        ["ui-screenshots", options @ ..] => ui_screenshots::run(options),
        ["cross-decode", files @ ..] => {
            let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
            cross_decode::run(&files)
        }
        _ => {
            eprintln!("usage: {}", USAGE_LINES.join("\n       "));
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
