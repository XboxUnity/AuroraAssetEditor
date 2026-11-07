mod crafted;
mod cross_decode;
mod dds_corpus;
mod golden;
mod synthetic;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask golden <generate [--refresh-inputs] | check>\n       cargo xtask cross-decode [<image files>...]\n       cargo xtask cross-samples";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = match arguments.as_slice() {
        ["golden", "generate"] => golden::generate(false),
        ["golden", "generate", "--refresh-inputs"] => golden::generate(true),
        ["golden", "check"] => golden::check(),
        ["cross-samples"] => cross_decode::write_samples(),
        ["cross-decode", files @ ..] => {
            let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
            cross_decode::run(&files)
        }
        _ => {
            eprintln!("{USAGE}");
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
