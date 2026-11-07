mod support;

use aurora_asset::dds::{self, DdsError};
use support::DdsSample;

const BYTES_PER_PIXEL: usize = 4;
const HEIGHT_OFFSET: usize = 12;
const WIDTH_OFFSET: usize = 16;

fn samples() -> Vec<DdsSample> {
    let samples = support::manifest().dds;
    assert!(!samples.is_empty());
    samples
}

/// Samples whose DLL output the port reproduces, so they carry hashes.
fn reproduced_samples() -> Vec<DdsSample> {
    samples()
        .into_iter()
        .filter(|sample| !sample.dll_output_unstable && !sample.dll_layout_scrambled)
        .collect()
}

fn header_size(bytes: &[u8]) -> (u32, u32) {
    let read = |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    (read(WIDTH_OFFSET), read(HEIGHT_OFFSET))
}

#[test]
fn every_sample_is_accepted_by_the_dll() {
    for sample in samples() {
        assert!(sample.dll_accepted, "{}", sample.name);
    }
}

#[test]
fn dds_matches_dll() {
    let samples = reproduced_samples();
    assert!(!samples.is_empty());
    for sample in samples {
        assert!(!sample.decoded_sha256.is_empty(), "{}", sample.name);
        let decoded = dds::decode(&support::dds_bytes(&sample)).unwrap();
        assert_eq!(
            support::rgba_sha256(decoded.as_raw()),
            sample.decoded_sha256,
            "{}",
            sample.name
        );
    }
}

#[test]
fn dds_matches_dll_verbatim_over_the_reported_length() {
    let samples: Vec<_> = reproduced_samples()
        .into_iter()
        .filter(|sample| !sample.verbatim_sha256.is_empty())
        .collect();
    assert!(!samples.is_empty());
    for sample in samples {
        let decoded = dds::decode(&support::dds_bytes(&sample)).unwrap();
        let prefix = &decoded.as_raw()[..sample.reported_len];
        assert_eq!(
            support::rgba_sha256(prefix),
            sample.verbatim_sha256,
            "{}",
            sample.name
        );
    }
}

#[test]
fn scrambled_dll_layouts_decode_to_the_header_size() {
    let samples: Vec<_> = samples()
        .into_iter()
        .filter(|sample| sample.dll_layout_scrambled)
        .collect();
    assert!(!samples.is_empty());
    for sample in samples {
        let bytes = support::dds_bytes(&sample);
        let decoded = dds::decode(&bytes).unwrap();
        assert_eq!(decoded.dimensions(), header_size(&bytes), "{}", sample.name);
    }
}

#[test]
fn unstable_dll_formats_are_unsupported() {
    let samples: Vec<_> = samples()
        .into_iter()
        .filter(|sample| sample.dll_output_unstable)
        .collect();
    assert!(!samples.is_empty());
    for sample in samples {
        let result = dds::decode(&support::dds_bytes(&sample));
        assert!(
            matches!(result, Err(DdsError::Unsupported(_))),
            "{}: {result:?}",
            sample.name
        );
    }
}

#[test]
fn x8r8g8b8_decodes_opaque() {
    let sample = reproduced_samples()
        .into_iter()
        .find(|sample| sample.name == "x8r8g8b8_16x16")
        .unwrap();
    let decoded = dds::decode(&support::dds_bytes(&sample)).unwrap();
    assert!(
        decoded
            .as_raw()
            .as_chunks::<BYTES_PER_PIXEL>()
            .0
            .iter()
            .all(|pixel| pixel[3] == u8::MAX)
    );
}

#[test]
fn rejects_non_dds() {
    assert_eq!(dds::decode(b"not a dds file at all"), Err(DdsError::NotDds));
    assert_eq!(dds::decode(&[]), Err(DdsError::NotDds));
}

#[test]
fn rejects_truncated() {
    for sample in reproduced_samples() {
        let bytes = support::dds_bytes(&sample);
        assert_eq!(dds::decode(&bytes[..60]), Err(DdsError::Truncated));
        let top_level_only = !sample.name.contains("_mips");
        if top_level_only {
            assert_eq!(
                dds::decode(&bytes[..bytes.len() - 1]),
                Err(DdsError::Truncated),
                "{}",
                sample.name
            );
        }
    }
}
