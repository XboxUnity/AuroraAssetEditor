mod support;

use aurora_asset::{
    Compression, DecodeError, EncodeError, EncodedTexture, MAX_DIMENSION, RgbaImage, TextureHeader,
    decode, encode,
};

#[test]
fn argb_encode_matches_dll() {
    for fixture in support::manifest().fixture {
        let encoded = encode(&support::input_rgba(&fixture.name), Compression::None).unwrap();
        assert_eq!(
            encoded.header.to_bytes(),
            support::header_bytes(&fixture.argb_header),
            "{} header",
            fixture.name
        );
        assert_eq!(
            support::rgba_sha256(&encoded.data),
            fixture.argb_video_sha256,
            "{} data",
            fixture.name
        );
        if fixture.argb_video_stored {
            assert_eq!(
                encoded.data,
                support::argb_video(&fixture.name),
                "{} stored video",
                fixture.name
            );
        }
    }
}

#[test]
fn argb_decode_matches_dll() {
    for fixture in support::manifest().fixture {
        let encoded = encode(&support::input_rgba(&fixture.name), Compression::None).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (fixture.width, fixture.height)
        );
        assert_eq!(
            support::rgba_sha256(decoded.as_raw()),
            fixture.argb_decoded_sha256,
            "{}",
            fixture.name
        );
    }
}

#[test]
fn argb_decode_needs_only_the_bytes_the_dll_reads() {
    for fixture in support::manifest().fixture {
        assert!(
            fixture.argb_decodes_at_minimum_len,
            "{}: the DLL decodes the shortened video differently",
            fixture.name
        );
        let encoded = encode(&support::input_rgba(&fixture.name), Compression::None).unwrap();
        support::assert_decodes_at_minimum_len(
            &encoded,
            support::ARGB_TEXEL,
            &fixture.argb_decoded_sha256,
            &fixture.name,
        );
    }
}

#[test]
fn rejects_zero_and_oversized_dimensions() {
    for (width, height) in [(0, 10), (MAX_DIMENSION + 1, 1)] {
        let image = RgbaImage::new(width, height);
        let error = encode(&image, Compression::None).unwrap_err();
        assert_eq!(error, EncodeError::DimensionsOutOfRange { width, height });
        assert_eq!(
            error.to_string(),
            format!(
                "image size {width}x{height} is outside the supported range of 1 to \
                 {MAX_DIMENSION} pixels per side"
            )
        );
    }
}

#[test]
fn decode_rejects_short_data() {
    let mut encoded = encode(&RgbaImage::new(64, 64), Compression::None).unwrap();
    let full = encoded.data.len();
    encoded.data.truncate(full / 2);
    assert_eq!(
        decode(&encoded).unwrap_err(),
        DecodeError::TruncatedData {
            expected: full,
            actual: full / 2
        }
    );
}

#[test]
fn decode_rejects_unsupported_format() {
    let mut bytes = encode(&RgbaImage::new(4, 4), Compression::None)
        .unwrap()
        .header
        .to_bytes();
    bytes[32..36].copy_from_slice(&0x0000_003Fu32.to_be_bytes());
    let texture = EncodedTexture {
        header: TextureHeader::from_bytes(bytes),
        data: Vec::new(),
    };
    assert!(matches!(
        decode(&texture),
        Err(DecodeError::UnsupportedFormat(_))
    ));
}
