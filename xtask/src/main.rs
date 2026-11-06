mod crafted;
mod golden;
mod synthetic;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask golden <generate [--refresh-inputs] | check>";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = match arguments.as_slice() {
        ["golden", "generate"] => golden::generate(false),
        ["golden", "generate", "--refresh-inputs"] => golden::generate(true),
        ["golden", "check"] => golden::check(),
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
