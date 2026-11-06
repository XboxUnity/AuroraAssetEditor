#![cfg(all(windows, target_arch = "x86"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SIZE: u32 = 256;
const DXT_VIDEO_LEN: usize = 65_536;
const UNCOMPRESSED_VIDEO_LEN: usize = 262_144;
const DECODED_LEN: usize = 262_144;
const DXT_HEADER: [u32; 13] = [
    0x0000_0003,
    0x0000_0001,
    0,
    0,
    0,
    0xFFFF_0000,
    0xFFFF_0000,
    0x0200_0002,
    0x0000_0054,
    0x001F_E0FF,
    0x0000_0D10,
    0,
    0x0000_0A00,
];
const FORMAT_DWORD_INDEX: usize = 8;
const SIZE_DWORD_INDEX: usize = 10;
const UNCOMPRESSED_FORMAT_DWORD: u32 = 0x0000_0086;
const UNCOMPRESSED_SIZE_DWORD: u32 = 0x0000_0C14;

fn dll_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dll/AuroraAsset.dll")
}

fn big_endian_dwords(bytes: &[u8]) -> [u32; 13] {
    let mut dwords = [0u32; 13];
    let (chunks, _) = bytes.as_chunks::<4>();
    for (dword, chunk) in dwords.iter_mut().zip(chunks) {
        *dword = u32::from_be_bytes(*chunk);
    }
    dwords
}

fn gradient_argb() -> Vec<u8> {
    let mut pixels = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            pixels.extend_from_slice(&[255, x as u8, y as u8, ((x + y) / 2) as u8]);
        }
    }
    pixels
}

fn run_harness(args: &[&Path]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_golden-harness"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "harness failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn encode(dir: &Path, input: &Path, compression: &str) -> (Vec<u8>, Vec<u8>) {
    let header_path = dir.join(format!("{compression}.hdr"));
    let video_path = dir.join(format!("{compression}.vid"));
    let size = SIZE.to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_golden-harness"))
        .args(["encode", "--dll"])
        .arg(dll_path())
        .arg("--input")
        .arg(input)
        .args(["--width", &size, "--height", &size])
        .args(["--compression", compression])
        .arg("--header-out")
        .arg(&header_path)
        .arg("--video-out")
        .arg(&video_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "encode failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        std::fs::read(header_path).unwrap(),
        std::fs::read(video_path).unwrap(),
    )
}

#[test]
fn encodes_and_decodes_gradient() {
    let dir = std::env::temp_dir().join(format!("golden-harness-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("gradient.argb");
    std::fs::write(&input, gradient_argb()).unwrap();

    let (header, video) = encode(&dir, &input, "dxt");
    assert_eq!(video.len(), DXT_VIDEO_LEN);
    assert_eq!(big_endian_dwords(&header), DXT_HEADER);

    let (header, video) = encode(&dir, &input, "none");
    assert_eq!(video.len(), UNCOMPRESSED_VIDEO_LEN);
    let dwords = big_endian_dwords(&header);
    assert_eq!(dwords[FORMAT_DWORD_INDEX], UNCOMPRESSED_FORMAT_DWORD);
    assert_eq!(dwords[SIZE_DWORD_INDEX], UNCOMPRESSED_SIZE_DWORD);

    let (dxt_header, dxt_video) = encode(&dir, &input, "dxt");
    let header_path = dir.join("dxt.hdr");
    let video_path = dir.join("dxt.vid");
    std::fs::write(&header_path, dxt_header).unwrap();
    std::fs::write(&video_path, dxt_video).unwrap();
    let decoded_path = dir.join("decoded.bgra");
    let output = run_harness(&[
        Path::new("decode"),
        Path::new("--dll"),
        &dll_path(),
        Path::new("--header"),
        &header_path,
        Path::new("--video"),
        &video_path,
        Path::new("--out"),
        &decoded_path,
    ]);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "256 256");
    assert_eq!(std::fs::read(decoded_path).unwrap().len(), DECODED_LEN);

    std::fs::remove_dir_all(&dir).unwrap();
}
