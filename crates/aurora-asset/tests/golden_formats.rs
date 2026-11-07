mod support;

use aurora_asset::{
    Compression, DecodeError, EncodedTexture, TEXTURE_HEADER_LEN, TextureFormat, TextureHeader,
    decode,
};

/// Crafted cases the DLL decodes but the port refuses, matched by a name fragment.
/// Anything that is not listed here has to decode exactly as the DLL does.
const EXPECTED_REFUSALS: [&str; 5] = [
    "dxt2_3",
    "_tiled_",
    "_gamma_",
    "_probe_border_",
    "_swz0124_",
];

/// How many crafted cases the DLL accepts and the port refuses: a case that stops being refused
/// must be noticed.
const EXPECTED_REFUSAL_COUNT: usize = 23;

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

fn surface_unit(format: TextureFormat) -> support::SurfaceUnit {
    match format {
        TextureFormat::LinearDxt1 => support::BC1_BLOCK,
        TextureFormat::LinearDxt4 | TextureFormat::LinearDxt5 => support::BC3_BLOCK,
        TextureFormat::LinearArgb8888 => support::ARGB_TEXEL,
        TextureFormat::Unsupported { .. } => panic!("an unsupported format has no surface unit"),
    }
}

fn is_expected_refusal(name: &str) -> bool {
    EXPECTED_REFUSALS
        .iter()
        .any(|fragment| name.contains(fragment))
}

#[test]
fn crafted_cases_match_dll() {
    let mut refused = 0;
    for case in support::manifest().crafted {
        let texture = crafted_texture(&case);
        if !case.dll_accepted {
            assert!(
                matches!(decode(&texture), Err(DecodeError::UnsupportedFormat(_))),
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
                assert_eq!(
                    support::rgba_sha256(decoded.as_raw()),
                    case.decoded_sha256,
                    "{}",
                    case.name
                );
                assert_eq!(
                    case.decodes_at_minimum_len,
                    Some(true),
                    "{}: the DLL decodes the shortened video differently",
                    case.name
                );
                support::assert_decodes_at_minimum_len(
                    &texture,
                    surface_unit(texture.header.format()),
                    &case.decoded_sha256,
                    &case.name,
                );
            }
        }
    }
    assert_eq!(refused, EXPECTED_REFUSAL_COUNT);
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

#[test]
fn stored_crafted_decodes_match_their_manifest_hashes() {
    for case in support::manifest().crafted {
        if case.dll_accepted {
            assert_eq!(
                support::rgba_sha256(&support::crafted_decoded_rgba(&case)),
                case.decoded_sha256,
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn cases_stored_as_equal_to_another_have_its_hash() {
    let manifest = support::manifest();
    for case in &manifest.crafted {
        let Some(baseline_name) = &case.same_as else {
            continue;
        };
        let baseline = manifest
            .crafted
            .iter()
            .find(|other| &other.name == baseline_name)
            .unwrap();
        assert_eq!(
            case.decoded_sha256, baseline.decoded_sha256,
            "{}",
            case.name
        );
    }
}

#[test]
fn unmodelled_fields_the_dll_depends_on_must_match_its_template() {
    const COMMON_OFFSET: usize = 3;
    const FETCH_3_OFFSET: usize = 40;
    const FETCH_5_OFFSET: usize = 50;
    let template = TextureHeader::for_image(64, 64, Compression::Dxt)
        .unwrap()
        .to_bytes();
    assert_eq!(
        TextureHeader::from_bytes(template).format(),
        TextureFormat::LinearDxt4
    );
    let tampered = [
        (COMMON_OFFSET, 0b0010),
        (FETCH_3_OFFSET, 0x80),
        (FETCH_5_OFFSET, 0x04),
    ];
    for (offset, bits) in tampered {
        let mut bytes = template;
        bytes[offset] ^= bits;
        assert_eq!(
            TextureHeader::from_bytes(bytes).format(),
            TextureFormat::Unsupported {
                data_format: 0x14,
                tiled: false
            },
            "byte {offset} xor {bits:#x}"
        );
    }
}
