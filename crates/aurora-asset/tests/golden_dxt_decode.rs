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

#[test]
fn stored_dll_decodes_match_their_manifest_hashes() {
    for fixture in support::manifest().fixture {
        if fixture.dxt_decoded_stored {
            assert_eq!(
                support::rgba_sha256(&support::dxt_decoded_rgba(&fixture.name)),
                fixture.dxt_decoded_sha256,
                "{}",
                fixture.name
            );
        }
    }
}

#[test]
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

fn expand_565_channels(packed: u16) -> [f64; 3] {
    let red = f64::from(packed >> 11);
    let green = f64::from((packed >> 5) & 0b11_1111);
    let blue = f64::from(packed & 0b1_1111);
    [
        red * 255.0 / 31.0,
        green * 255.0 / 63.0,
        blue * 255.0 / 31.0,
    ]
}

#[test]
fn semi_transparent_pixels_brighten() {
    const GREY_565: u16 = (18 << 11) | (37 << 5) | 18;
    const ALPHA: u8 = 200;
    let mut texture = zeroed_dxt_texture(4, 4, 32 * 32 * 16);
    // One BC3 block: both alpha endpoints 200, both colour endpoints grey (about 150),
    // all indices 0. Words are stored byte-swapped (8in16).
    let mut block = [0u8; 16];
    block[0] = ALPHA;
    block[1] = ALPHA;
    block[8..10].copy_from_slice(&GREY_565.to_le_bytes());
    block[10..12].copy_from_slice(&GREY_565.to_le_bytes());
    for word in block.as_chunks_mut::<2>().0.iter_mut() {
        word.reverse();
    }
    texture.data[..16].copy_from_slice(&block);

    let [red, green, blue] = expand_565_channels(GREY_565)
        .map(|channel| (channel * 255.0 / 200.0).round_ties_even() as u8);
    assert!(red > 150 && red < 255, "the divided colour must not clamp");
    let decoded = decode(&texture).unwrap();
    assert!(
        decoded
            .pixels()
            .all(|pixel| pixel.0 == [red, green, blue, ALPHA])
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
