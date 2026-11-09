use std::path::{Path, PathBuf};

use aurora_image::{ImageTarget, RESIZE_FILTER, decode_image, encoder_pixels, fit_to_target};

const BYTES_PER_PIXEL: usize = 4;
const CHOSEN_FILTER_PREFIX: &str = "Chosen filter: ";

/// The target-size gate set, written by `cargo xtask closeness-inputs`.
const GATE_INPUTS: [(&str, ImageTarget); 14] = [
    ("bmp_24.bmp", ImageTarget::Icon),
    ("bmp_32.bmp", ImageTarget::Icon),
    ("png_16bit.png", ImageTarget::Icon),
    ("png_banner.png", ImageTarget::Banner),
    ("png_gamma_1_0.png", ImageTarget::Icon),
    ("png_gray.png", ImageTarget::Icon),
    ("png_gray16.png", ImageTarget::Icon),
    ("png_gray_alpha.png", ImageTarget::Icon),
    ("png_iccp.png", ImageTarget::Icon),
    ("png_palette.png", ImageTarget::Icon),
    ("png_palette_trns.png", ImageTarget::Icon),
    ("png_rgb.png", ImageTarget::Icon),
    ("png_rgba.png", ImageTarget::Icon),
    ("png_srgb_chunk.png", ImageTarget::Icon),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn closeness_directory() -> PathBuf {
    repository_root().join("fixtures/image/closeness")
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// What the new app hands the encoder for a file loaded with Auto Resize on, as A, R, G, B.
fn pipeline_argb(file: &str, target: ImageTarget) -> Vec<u8> {
    let bytes = read(&closeness_directory().join("inputs").join(file));
    let decoded = decode_image(&bytes, Some(file)).expect("gate inputs decode");
    let fitted = fit_to_target(decoded, target, true);
    encoder_pixels(&fitted)
        .pixels()
        .flat_map(|pixel| {
            let [red, green, blue, alpha] = pixel.0;
            [alpha, red, green, blue]
        })
        .collect()
}

fn reference_argb(file: &str, target: ImageTarget) -> Vec<u8> {
    let stem = Path::new(file)
        .file_stem()
        .expect("gate inputs have a name")
        .to_string_lossy();
    let (width, height) = target.size();
    read(
        &closeness_directory()
            .join("gdiplus")
            .join(format!("{stem}.{width}x{height}.argb")),
    )
}

fn describe_mismatch(file: &str, ours: &[u8], gdiplus: &[u8]) -> String {
    if ours.len() != gdiplus.len() {
        return format!("{file}: {} bytes, GDI+ {} bytes", ours.len(), gdiplus.len());
    }
    let differing: Vec<usize> = ours
        .as_chunks::<BYTES_PER_PIXEL>()
        .0
        .iter()
        .zip(gdiplus.as_chunks::<BYTES_PER_PIXEL>().0)
        .enumerate()
        .filter_map(|(index, (ours, gdiplus))| (ours != gdiplus).then_some(index))
        .collect();
    let first = differing[0] * BYTES_PER_PIXEL;
    format!(
        "{file}: {} pixels differ, first at index {}: ours {:?}, GDI+ {:?}",
        differing.len(),
        differing[0],
        &ours[first..first + BYTES_PER_PIXEL],
        &gdiplus[first..first + BYTES_PER_PIXEL]
    )
}

#[test]
fn png_and_bmp_at_target_size_match_gdiplus() {
    let mismatches: Vec<String> = GATE_INPUTS
        .iter()
        .filter_map(|&(file, target)| {
            let ours = pipeline_argb(file, target);
            let gdiplus = reference_argb(file, target);
            (ours != gdiplus).then(|| describe_mismatch(file, &ours, &gdiplus))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn resize_filter_matches_report() {
    let report_path = repository_root().join("docs/image/gdiplus-closeness.md");
    let report = String::from_utf8(read(&report_path)).expect("the report is UTF-8");
    let chosen = report
        .lines()
        .find_map(|line| line.strip_prefix(CHOSEN_FILTER_PREFIX))
        .expect("the report names the chosen filter");
    assert_eq!(chosen, RESIZE_FILTER.name());
}
