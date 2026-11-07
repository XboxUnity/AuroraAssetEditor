use crate::aurora_asset_dll::AuroraAssetDll;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 2;
const EXIT_ENCODE_REFUSED: i32 = 4;
const EXIT_DECODE_REFUSED: i32 = 5;
const EXIT_DDS_REFUSED: i32 = 6;

const USAGE: &str = "usage:
  golden-harness encode --dll <path> --input <file.argb> --width <u32> --height <u32> --compression dxt|none --header-out <file> --video-out <file>
  golden-harness decode --dll <path> --header <file> --video <file> --out <file.bgra>
  golden-harness decode-dds --dll <path> --input <file.dds> --out <file.bgra>";

type Options = HashMap<String, String>;

/// `Ok` carries the exit code; `Err` carries a message for stderr and exits with 2.
type CliResult = Result<i32, String>;

pub fn run(args: Vec<String>) -> i32 {
    match dispatch(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            EXIT_USAGE
        }
    }
}

fn dispatch(args: &[String]) -> CliResult {
    let (subcommand, rest) = args.split_first().ok_or(USAGE)?;
    let options = parse_options(rest)?;
    match subcommand.as_str() {
        "encode" => encode(&options),
        "decode" => decode(&options),
        "decode-dds" => decode_dds(&options),
        other => Err(format!("unknown subcommand '{other}'\n{USAGE}")),
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options::new();
    let mut remaining = args.iter();
    while let Some(flag) = remaining.next() {
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| format!("expected an option, found '{flag}'\n{USAGE}"))?;
        let value = remaining
            .next()
            .ok_or_else(|| format!("option '{flag}' needs a value"))?;
        options.insert(name.to_owned(), value.clone());
    }
    Ok(options)
}

fn required<'a>(options: &'a Options, name: &str) -> Result<&'a str, String> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing option --{name}\n{USAGE}"))
}

fn required_path(options: &Options, name: &str) -> Result<PathBuf, String> {
    required(options, name).map(PathBuf::from)
}

fn required_u32(options: &Options, name: &str) -> Result<u32, String> {
    let text = required(options, name)?;
    text.parse()
        .map_err(|error| format!("--{name} '{text}' is not a u32: {error}"))
}

fn load_dll(options: &Options) -> Result<AuroraAssetDll, String> {
    let path = required_path(options, "dll")?;
    AuroraAssetDll::load(&path).map_err(|error| format!("cannot load {}: {error}", path.display()))
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn parse_compression(text: &str) -> Result<bool, String> {
    match text {
        "dxt" => Ok(true),
        "none" => Ok(false),
        other => Err(format!(
            "--compression must be dxt or none, found '{other}'"
        )),
    }
}

fn encode(options: &Options) -> CliResult {
    let dll = load_dll(options)?;
    let argb = read_file(&required_path(options, "input")?)?;
    let width = required_u32(options, "width")?;
    let height = required_u32(options, "height")?;
    let compress = parse_compression(required(options, "compression")?)?;
    let header_out = required_path(options, "header-out")?;
    let video_out = required_path(options, "video-out")?;

    let Some((header, video)) = dll.image_to_asset(&argb, width, height, compress) else {
        eprintln!("the DLL refused to encode the image");
        return Ok(EXIT_ENCODE_REFUSED);
    };
    write_file(&header_out, &header)?;
    write_file(&video_out, &video)?;
    Ok(EXIT_OK)
}

fn decode(options: &Options) -> CliResult {
    let dll = load_dll(options)?;
    let header = read_file(&required_path(options, "header")?)?;
    let video = read_file(&required_path(options, "video")?)?;
    let out = required_path(options, "out")?;

    let Some((bgra, width, height)) = dll.asset_to_image(&header, &video) else {
        eprintln!("the DLL refused to decode the asset");
        return Ok(EXIT_DECODE_REFUSED);
    };
    write_file(&out, &bgra)?;
    println!("{width} {height}");
    Ok(EXIT_OK)
}

fn decode_dds(options: &Options) -> CliResult {
    let dll = load_dll(options)?;
    let dds = read_file(&required_path(options, "input")?)?;
    let out = required_path(options, "out")?;

    let Some((bgra, width, height, reported_len)) = dll.dds_to_image(&dds) else {
        eprintln!("the DLL refused to decode the DDS");
        return Ok(EXIT_DDS_REFUSED);
    };
    write_file(&out, &bgra)?;
    println!("{width} {height} {reported_len}");
    Ok(EXIT_OK)
}
