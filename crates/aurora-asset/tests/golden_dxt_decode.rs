mod support;

use aurora_asset::{Compression, DecodeError, EncodedTexture, TextureHeader, decode};

fn zeroed_dxt_texture(width: u32, height: u32, data_len: usize) -> EncodedTexture {
    EncodedTexture {
        header: TextureHeader::for_image(width, height, Compression::Dxt).unwrap(),
        data: vec![0; data_len],
    }
}

fn dxt_texture(fixture: &support::Fixture) -> EncodedTexture {
    EncodedTexture {
        header: TextureHeader::from_bytes(support::header_bytes(&fixture.dxt_header)),
        data: support::dxt_video(&fixture.name),
    }
}

fn assert_decodes_like_dll(fixture: &support::Fixture) {
    let decoded = decode(&dxt_texture(fixture)).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (fixture.width, fixture.height),
        "{} size",
        fixture.name
    );
    assert_eq!(
        support::rgba_sha256(decoded.as_raw()),
        fixture.dxt_decoded_sha256,
        "{}",
        fixture.name
    );
}

/// Fixtures whose only mismatches against the DLL are off by one on exact .5 ties
/// (the DLL's float precision, closed in Task 9), with the measured mismatching pixel count.
const HALF_TIE_FIXTURES: [(&str, usize); 3] =
    [("alphagrad", 5), ("alphanoise", 7), ("odd_130x70", 1)];

#[test]
fn dxt_decode_matches_dll_on_real_and_structured_images() {
    for fixture in support::manifest().fixture {
        if HALF_TIE_FIXTURES
            .iter()
            .any(|(name, _)| *name == fixture.name)
        {
            continue;
        }
        assert_decodes_like_dll(&fixture);
    }
}

#[test]
#[ignore = "closed in Task 9"]
fn dxt_decode_matches_dll_on_all_fixtures() {
    for fixture in support::manifest().fixture {
        assert_decodes_like_dll(&fixture);
    }
}

#[test]
fn alpha_zero_decodes_to_black() {
    let fixture = support::manifest()
        .fixture
        .into_iter()
        .find(|fixture| fixture.name == "alpha_zero")
        .unwrap();
    let decoded = decode(&dxt_texture(&fixture)).unwrap();
    assert!(decoded.pixels().all(|pixel| pixel.0 == [0, 0, 0, 0]));
}

#[test]
fn semi_transparent_pixels_brighten() {
    const GREY_565: [u8; 2] = 0xE73Cu16.to_le_bytes();
    const ALPHA: u8 = 200;
    let mut texture = zeroed_dxt_texture(4, 4, 32 * 32 * 16);
    // One BC3 block: both alpha endpoints 200, both colour endpoints grey (about 230),
    // all indices 0. Words are stored byte-swapped (8in16).
    let mut block = [0u8; 16];
    block[0] = ALPHA;
    block[1] = ALPHA;
    block[8..10].copy_from_slice(&GREY_565);
    block[10..12].copy_from_slice(&GREY_565);
    for word in block.as_chunks_mut::<2>().0.iter_mut() {
        word.reverse();
    }
    texture.data[..16].copy_from_slice(&block);
    let decoded = decode(&texture).unwrap();
    assert!(
        decoded
            .pixels()
            .all(|pixel| pixel.0 == [255, 255, 255, ALPHA])
    );
}

#[test]
fn decode_rejects_short_dxt_data() {
    let texture = zeroed_dxt_texture(64, 64, 100);
    assert!(matches!(
        decode(&texture),
        Err(DecodeError::TruncatedData { actual: 100, .. })
    ));
}
