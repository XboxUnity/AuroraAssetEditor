mod support;

use aurora_asset::{
    Compression, DecodeError, EncodedTexture, TEXTURE_HEADER_LEN, TextureFormat, TextureHeader,
    decode, unrounded_values,
};

/// Crafted cases the DLL decodes but the port refuses, matched by a name fragment.
/// Anything that is not listed here has to decode exactly as the DLL does.
const EXPECTED_REFUSALS: [&str; 4] = ["dxt2_3", "_tiled_", "_abgr_", "_gamma_"];

/// Crafted cases whose only mismatches against the DLL are off by one on exact .5 ties
/// (the DLL's float precision, closed in Task 9), with the exact count of mismatching pixels.
const HALF_TIE_CASES: [(&str, usize); 20] = [
    ("dxt1_e0_rgba_32x32", 7),
    ("dxt1_e0_bgra_32x32", 7),
    ("dxt1_e1_rgba_32x32", 7),
    ("dxt1_e1_bgra_32x32", 7),
    ("dxt1_e2_rgba_32x32", 6),
    ("dxt1_e2_bgra_32x32", 6),
    ("dxt1_e3_rgba_32x32", 2),
    ("dxt1_e3_bgra_32x32", 2),
    ("dxt4_5_e0_rgba_32x32", 6),
    ("dxt4_5_e0_bgra_32x32", 6),
    ("dxt4_5_e2_rgba_32x32", 3),
    ("dxt4_5_e2_bgra_32x32", 3),
    ("dxt1_e1_rgba_130x70", 52),
    ("dxt1_e1_bgra_130x70", 52),
    ("dxt4_5_e1_rgba_130x70", 7),
    ("dxt4_5_e1_bgra_130x70", 67),
    ("dxt1_e1_rgba_n1_32x32", 7),
    ("dxt1_e1_rgba_signed_32x32", 7),
    ("dxt1_e1_rgba_pitch12_130x70", 39),
    ("dxt4_5_e1_rgba_pitch12_130x70", 2),
];

/// A mathematically exact .5 comes out of the f64 arithmetic with noise around 1e-14.
const TIE_TOLERANCE: f64 = 1e-9;

fn crafted_texture(case: &support::Crafted) -> EncodedTexture {
    EncodedTexture {
        header: TextureHeader::from_bytes(support::header_bytes(&case.header)),
        data: support::crafted_video(case),
    }
}

fn crafted_case(name: &str) -> support::Crafted {
    support::manifest()
        .crafted
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("no crafted case named {name}"))
}

fn format_of(name: &str) -> TextureFormat {
    TextureHeader::from_bytes(support::header_bytes(&crafted_case(name).header)).format()
}

fn is_expected_refusal(name: &str) -> bool {
    EXPECTED_REFUSALS
        .iter()
        .any(|fragment| name.contains(fragment))
}

/// The pixels where `ours` differs from the DLL's decode, after proving each difference is an
/// off-by-one on an exact .5 tie. Panics on any other difference.
fn count_tie_mismatches(case: &support::Crafted, ours: &[u8], unrounded: &[[f64; 4]]) -> usize {
    let dll = support::crafted_decoded_rgba(case);
    assert_eq!(dll.len(), ours.len(), "{} decoded size", case.name);
    let mut mismatching_pixels = 0;
    for (pixel, ((ours, dll), values)) in ours
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dll.as_chunks::<4>().0.iter())
        .zip(unrounded)
        .enumerate()
    {
        if ours == dll {
            continue;
        }
        mismatching_pixels += 1;
        for channel in 0..4 {
            if ours[channel] == dll[channel] {
                continue;
            }
            assert_eq!(
                ours[channel].abs_diff(dll[channel]),
                1,
                "{} pixel {pixel} channel {channel}",
                case.name
            );
            let fraction = values[channel] - values[channel].floor();
            assert!(
                (fraction - 0.5).abs() < TIE_TOLERANCE,
                "{} pixel {pixel} channel {channel} is not a .5 tie ({})",
                case.name,
                values[channel]
            );
        }
    }
    mismatching_pixels
}

#[test]
fn crafted_cases_match_dll() {
    let mut tie_cases = Vec::new();
    let mut refused = 0;
    for case in support::manifest().crafted {
        let texture = crafted_texture(&case);
        if !case.dll_accepted {
            assert!(
                decode(&texture).is_err(),
                "{} is refused by the DLL",
                case.name
            );
            continue;
        }
        match (decode(&texture), is_expected_refusal(&case.name)) {
            (Err(error), true) => {
                assert!(
                    matches!(error, DecodeError::UnsupportedFormat(_)),
                    "{}: {error}",
                    case.name
                );
                refused += 1;
            }
            (Err(error), false) => panic!("{} was refused: {error}", case.name),
            (Ok(_), true) => panic!("{} should be refused", case.name),
            (Ok(decoded), false) => {
                let unrounded = unrounded_values(&texture).unwrap();
                let mismatches = count_tie_mismatches(&case, decoded.as_raw(), &unrounded);
                if mismatches > 0 {
                    tie_cases.push((case.name.clone(), mismatches));
                }
            }
        }
    }
    println!("refused by the port: {refused}; tie cases: {tie_cases:?}");
    let expected: Vec<(String, usize)> = HALF_TIE_CASES
        .iter()
        .map(|(name, count)| (name.to_string(), *count))
        .collect();
    assert_eq!(tie_cases, expected);
}

#[test]
fn dxt1_header_reports_dxt1() {
    assert_eq!(format_of("dxt1_e1_rgba_32x32"), TextureFormat::LinearDxt1);
}

#[test]
fn reports_tiled_as_unsupported() {
    const FETCH_0_OFFSET: usize = 28;
    const TILED_BYTE_MASK: u8 = 0x80;
    let mut bytes = TextureHeader::for_image(64, 64, Compression::Dxt)
        .unwrap()
        .to_bytes();
    assert_eq!(bytes.len(), TEXTURE_HEADER_LEN);
    bytes[FETCH_0_OFFSET] |= TILED_BYTE_MASK;
    assert_eq!(
        TextureHeader::from_bytes(bytes).format(),
        TextureFormat::Unsupported {
            data_format: 0x14,
            tiled: true
        }
    );
}

#[test]
fn only_exact_dxt4_divides_alpha() {
    assert_eq!(format_of("dxt4_5_e1_rgba_32x32"), TextureFormat::LinearDxt4);
    let not_dxt4 = [
        "dxt4_5_e0_rgba_32x32",
        "dxt4_5_e1_bgra_32x32",
        "dxt4_5_e2_rgba_32x32",
        "dxt4_5_e3_rgba_32x32",
        "dxt4_5_e1_rgba_n1_32x32",
        "dxt4_5_e1_rgba_signed_32x32",
    ];
    for name in not_dxt4 {
        assert_eq!(format_of(name), TextureFormat::LinearDxt5, "{name}");
    }

    let dxt4 = crafted_case("dxt4_5_e1_rgba_32x32");
    let dxt5 = crafted_case("dxt4_5_e1_rgba_n1_32x32");
    assert_eq!(support::crafted_video(&dxt4), support::crafted_video(&dxt5));
    assert_ne!(
        support::crafted_decoded_rgba(&dxt4),
        support::crafted_decoded_rgba(&dxt5),
        "the DLL divides alpha for DXT4 only"
    );
}

#[test]
fn bc3_in_other_texture_formats_is_unsupported() {
    assert_eq!(
        format_of("dxt2_3_e1_rgba_32x32"),
        TextureFormat::Unsupported {
            data_format: 0x13,
            tiled: false
        }
    );
}
