mod support;

use aurora_asset::{Compression, EncodeError, MAX_DIMENSION, TextureFormat, TextureHeader};

#[test]
fn header_matches_dll_for_every_fixture() {
    for fixture in support::manifest().fixture {
        let cases = [
            (Compression::Dxt, &fixture.dxt_header),
            (Compression::None, &fixture.argb_header),
        ];
        for (compression, expected_hex) in cases {
            let header =
                TextureHeader::for_image(fixture.width, fixture.height, compression).unwrap();
            assert_eq!(
                header.to_bytes(),
                support::header_bytes(expected_hex),
                "{} {compression:?}",
                fixture.name
            );
        }
    }
}

#[test]
fn header_round_trips_width_height() {
    for (width, height) in [(1, 1), (63, 65), (1280, 720)] {
        let header = TextureHeader::for_image(width, height, Compression::Dxt).unwrap();
        assert_eq!((header.width(), header.height()), (width, height));
    }
}

#[test]
fn parsed_format_of_dll_headers() {
    for fixture in support::manifest().fixture {
        let dxt = TextureHeader::from_bytes(support::header_bytes(&fixture.dxt_header));
        let argb = TextureHeader::from_bytes(support::header_bytes(&fixture.argb_header));
        assert_eq!(dxt.format(), TextureFormat::LinearDxt4, "{}", fixture.name);
        assert_eq!(
            argb.format(),
            TextureFormat::LinearArgb8888,
            "{}",
            fixture.name
        );
    }
}

#[test]
fn unknown_fetch_constant_is_reported_as_unsupported() {
    let mut bytes = support::header_bytes(&support::manifest().fixture[0].dxt_header);
    bytes[32..36].copy_from_slice(&0x0000_003Fu32.to_be_bytes());
    assert_eq!(
        TextureHeader::from_bytes(bytes).format(),
        TextureFormat::Unsupported {
            data_format: 0x3F,
            tiled: false
        }
    );
}

#[test]
fn dimensions_outside_the_supported_range_are_rejected() {
    for (width, height) in [
        (0, 64),
        (64, 0),
        (MAX_DIMENSION + 1, 64),
        (64, MAX_DIMENSION + 1),
    ] {
        assert_eq!(
            TextureHeader::for_image(width, height, Compression::None),
            Err(EncodeError::DimensionsOutOfRange { width, height })
        );
    }
    assert!(TextureHeader::for_image(MAX_DIMENSION, MAX_DIMENSION, Compression::Dxt).is_ok());
}

#[test]
fn maximum_dimension_round_trips() {
    for (width, height) in [(MAX_DIMENSION, MAX_DIMENSION), (MAX_DIMENSION, 1)] {
        for compression in [Compression::Dxt, Compression::None] {
            let header = TextureHeader::for_image(width, height, compression).unwrap();
            assert_eq!((header.width(), header.height()), (width, height));
        }
    }
}
