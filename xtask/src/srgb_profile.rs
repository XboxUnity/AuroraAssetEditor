//! A minimal ICC v2 sRGB display profile, for the PNG `iCCP` closeness input.

const HEADER_LEN: usize = 128;
const TAG_ENTRY_LEN: usize = 12;
const CURVE_POINTS: u16 = 1024;
const D50_WHITE: [f64; 3] = [0.9642, 1.0, 0.8249];
/// sRGB primaries adapted to D50, as in the common sRGB profiles.
const RED_COLORANT: [f64; 3] = [0.4361, 0.2225, 0.0139];
const GREEN_COLORANT: [f64; 3] = [0.3851, 0.7169, 0.0971];
const BLUE_COLORANT: [f64; 3] = [0.1431, 0.0606, 0.7141];
const DESCRIPTION: &str = "sRGB";
const S15_FIXED_ONE: f64 = 65536.0;
const U16_MAX: f64 = 65535.0;
const TEXT_DESCRIPTION_SCRIPT_CODE_LEN: usize = 67;
const ICC_VERSION_2_1: u32 = 0x0210_0000;
const RENDERING_INTENT_PERCEPTUAL: u32 = 0;

/// sRGB's electro-optical transfer function.
fn srgb_to_linear(encoded: f64) -> f64 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

fn s15_fixed(value: f64) -> [u8; 4] {
    ((value * S15_FIXED_ONE).round() as i32).to_be_bytes()
}

fn xyz_tag(xyz: [f64; 3]) -> Vec<u8> {
    let mut tag = b"XYZ \0\0\0\0".to_vec();
    for component in xyz {
        tag.extend_from_slice(&s15_fixed(component));
    }
    tag
}

fn curve_tag() -> Vec<u8> {
    let mut tag = b"curv\0\0\0\0".to_vec();
    tag.extend_from_slice(&u32::from(CURVE_POINTS).to_be_bytes());
    let last = f64::from(CURVE_POINTS - 1);
    for point in 0..CURVE_POINTS {
        let linear = srgb_to_linear(f64::from(point) / last);
        tag.extend_from_slice(&((linear * U16_MAX).round() as u16).to_be_bytes());
    }
    tag
}

fn description_tag() -> Vec<u8> {
    let mut tag = b"desc\0\0\0\0".to_vec();
    let ascii_with_nul = DESCRIPTION.len() as u32 + 1;
    tag.extend_from_slice(&ascii_with_nul.to_be_bytes());
    tag.extend_from_slice(DESCRIPTION.as_bytes());
    tag.push(0);
    tag.extend_from_slice(&[0; 8]);
    tag.extend_from_slice(&[0; 3]);
    tag.extend_from_slice(&[0; TEXT_DESCRIPTION_SCRIPT_CODE_LEN]);
    tag
}

fn pad_to_four(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

fn header(profile_len: usize) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(&(profile_len as u32).to_be_bytes());
    // No preferred colour management module.
    header.extend_from_slice(&[0; 4]);
    header.extend_from_slice(&ICC_VERSION_2_1.to_be_bytes());
    header.extend_from_slice(b"mntrRGB XYZ ");
    // No creation date.
    header.extend_from_slice(&[0; 12]);
    header.extend_from_slice(b"acsp");
    // Platform, flags, device manufacturer and model, device attributes.
    header.extend_from_slice(&[0; 24]);
    header.extend_from_slice(&RENDERING_INTENT_PERCEPTUAL.to_be_bytes());
    for component in D50_WHITE {
        header.extend_from_slice(&s15_fixed(component));
    }
    header.resize(HEADER_LEN, 0);
    header
}

/// The profile bytes; red, green and blue share one tone curve.
pub fn srgb_profile() -> Vec<u8> {
    let curve = curve_tag();
    let tags: [(&[u8; 4], Vec<u8>); 5] = [
        (b"desc", description_tag()),
        (b"wtpt", xyz_tag(D50_WHITE)),
        (b"rXYZ", xyz_tag(RED_COLORANT)),
        (b"gXYZ", xyz_tag(GREEN_COLORANT)),
        (b"bXYZ", xyz_tag(BLUE_COLORANT)),
    ];
    let shared_curve_signatures: [&[u8; 4]; 3] = [b"rTRC", b"gTRC", b"bTRC"];
    let tag_count = tags.len() + shared_curve_signatures.len();

    let mut table = (tag_count as u32).to_be_bytes().to_vec();
    let mut data = Vec::new();
    let data_start = HEADER_LEN + 4 + tag_count * TAG_ENTRY_LEN;
    for (signature, tag) in &tags {
        table.extend_from_slice(*signature);
        table.extend_from_slice(&((data_start + data.len()) as u32).to_be_bytes());
        table.extend_from_slice(&(tag.len() as u32).to_be_bytes());
        data.extend_from_slice(tag);
        pad_to_four(&mut data);
    }
    let curve_offset = (data_start + data.len()) as u32;
    for signature in shared_curve_signatures {
        table.extend_from_slice(signature);
        table.extend_from_slice(&curve_offset.to_be_bytes());
        table.extend_from_slice(&(curve.len() as u32).to_be_bytes());
    }
    data.extend_from_slice(&curve);

    let mut profile = header(data_start + data.len());
    profile.extend_from_slice(&table);
    profile.extend_from_slice(&data);
    profile
}
