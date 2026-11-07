use std::io::Cursor;

use aurora_asset::RgbaImage;
use aurora_image::{ImportError, MAX_IMPORT_ALLOC, MAX_IMPORT_SIDE, decode_image};
use image::codecs::gif::GifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, Frame, ImageFormat, Rgba};

const WIDTH: u32 = 7;
const HEIGHT: u32 = 5;

const FOUR_COLOURS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 255, 255],
];

const OTHER_FOUR_COLOURS: [[u8; 4]; 4] = [
    [0, 0, 0, 255],
    [255, 255, 0, 255],
    [0, 255, 255, 255],
    [255, 0, 255, 255],
];

fn encode(image: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("test image encodes");
    bytes
}

fn encode_jpeg(image: RgbaImage, quality: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(&DynamicImage::ImageRgba8(image).to_rgb8())
        .expect("jpeg encodes");
    bytes
}

fn partial_alpha_image() -> RgbaImage {
    RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        Rgba([
            (x * 30) as u8,
            (y * 50) as u8,
            (x * y * 7) as u8,
            40 + (x * 5 + y * 20) as u8,
        ])
    })
}

fn four_colour_image(colours: [[u8; 4]; 4]) -> RgbaImage {
    RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| Rgba(colours[((x + y) % 4) as usize]))
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut checked = kind.to_vec();
    checked.extend_from_slice(data);
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(&checked);
    chunk.extend_from_slice(&crc32(&checked).to_be_bytes());
    chunk
}

/// A PNG signature and IHDR chunk, the part that carries the size.
fn png_header(width: u32, height: u32, bit_depth: u8, colour_type: u8) -> Vec<u8> {
    let mut data = width.to_be_bytes().to_vec();
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[bit_depth, colour_type, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(png_chunk(b"IHDR", &data));
    png
}

/// The IHDR plus an empty IDAT chunk: the PNG reader only reports the size once it reaches
/// the first IDAT, and no pixel data follows.
fn png_header_without_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut png = png_header(width, height, 8, 6);
    png.extend(png_chunk(b"IDAT", &[]));
    png
}

fn stored_zlib(data: &[u8]) -> Vec<u8> {
    let length = data.len() as u16;
    let mut stream = vec![0x78, 0x01, 0x01];
    stream.extend_from_slice(&length.to_le_bytes());
    stream.extend_from_slice(&(!length).to_le_bytes());
    stream.extend_from_slice(data);
    stream.extend_from_slice(&adler32(data).to_be_bytes());
    stream
}

/// An 8-bit PNG built from raw pixel bytes per row, each row filtered with type 0.
fn png_from_rows(
    width: u32,
    colour_type: u8,
    rows: &[&[u8]],
    chunks_before_data: &[(&[u8; 4], Vec<u8>)],
) -> Vec<u8> {
    let mut png = png_header(width, rows.len() as u32, 8, colour_type);
    for (kind, data) in chunks_before_data {
        png.extend(png_chunk(kind, data));
    }
    let raw: Vec<u8> = rows
        .iter()
        .flat_map(|row| std::iter::once(0u8).chain(row.iter().copied()))
        .collect();
    png.extend(png_chunk(b"IDAT", &stored_zlib(&raw)));
    png.extend(png_chunk(b"IEND", &[]));
    png
}

#[test]
fn decodes_every_lossless_import_format() {
    let source = partial_alpha_image();
    let formats = [
        ImageFormat::Png,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::WebP,
        ImageFormat::Tga,
        ImageFormat::Ico,
        ImageFormat::Qoi,
    ];
    for format in formats {
        let decoded = decode_image(&encode(&source, format), Some("x.tga"))
            .unwrap_or_else(|error| panic!("{format:?} failed: {error}"));
        assert_eq!(decoded, source, "{format:?}");
    }

    let gif_source = four_colour_image(FOUR_COLOURS);
    let decoded = decode_image(&encode(&gif_source, ImageFormat::Gif), None).expect("gif decodes");
    assert_eq!(decoded, gif_source, "Gif");
}

#[test]
fn jpeg_decodes_close_to_its_source() {
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        Rgba([(x * 8) as u8, (y * 8) as u8, ((x + y) * 4) as u8, 255])
    });

    let decoded = decode_image(&encode_jpeg(source.clone(), 95), None).expect("jpeg decodes");

    assert_eq!(decoded.dimensions(), source.dimensions());
    let largest_difference = decoded
        .pixels()
        .zip(source.pixels())
        .flat_map(|(a, b)| a.0.iter().zip(b.0.iter()).map(|(a, b)| a.abs_diff(*b)))
        .max()
        .expect("image has pixels");
    assert!(largest_difference < 16, "difference {largest_difference}");
}

#[test]
fn decodes_dds_with_the_codec_decoder() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/codec/dds/a8r8g8b8_16x16.dds"
    ))
    .expect("fixture exists");

    let decoded = decode_image(&bytes, None).expect("dds decodes");

    assert_eq!(
        decoded,
        aurora_asset::dds::decode(&bytes).expect("codec decodes")
    );
}

#[test]
fn tga_needs_the_file_name_hint() {
    let bytes = encode(&partial_alpha_image(), ImageFormat::Tga);

    assert!(matches!(
        decode_image(&bytes, None),
        Err(ImportError::UnknownFormat)
    ));
    assert_eq!(
        decode_image(&bytes, Some("x.TGA")).expect("tga decodes"),
        partial_alpha_image()
    );
}

#[test]
fn rejects_unknown_format() {
    assert!(matches!(
        decode_image(b"hello", None),
        Err(ImportError::UnknownFormat)
    ));
}

#[test]
fn rejects_oversized_image_before_decoding() {
    let png = png_header_without_pixels(20_000, 20_000);

    assert!(matches!(
        decode_image(&png, None),
        Err(ImportError::TooLarge {
            width: 20_000,
            height: 20_000
        })
    ));
}

#[test]
fn rejects_image_over_the_allocation_limit_before_decoding() {
    let png = png_header_without_pixels(12_000, 12_000);

    assert!(matches!(
        decode_image(&png, None),
        Err(ImportError::TooLarge {
            width: 12_000,
            height: 12_000
        })
    ));
}

#[test]
fn too_large_message_states_both_limits() {
    let message = ImportError::TooLarge {
        width: 20_000,
        height: 100,
    }
    .to_string();

    assert!(message.contains("20000x100"), "{message}");
    assert!(message.contains(&MAX_IMPORT_SIDE.to_string()), "{message}");
    assert!(
        message.contains(&format!("{} MiB", MAX_IMPORT_ALLOC / (1024 * 1024))),
        "{message}"
    );
}

#[test]
fn truncated_png_is_decode_error() {
    let bytes = encode(&partial_alpha_image(), ImageFormat::Png);

    assert!(matches!(
        decode_image(&bytes[..bytes.len() / 2], None),
        Err(ImportError::Decode(_))
    ));
}

#[test]
fn gif_takes_first_frame() {
    let first = four_colour_image(FOUR_COLOURS);
    let second = four_colour_image(OTHER_FOUR_COLOURS);
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder
            .encode_frames([Frame::new(first.clone()), Frame::new(second)])
            .expect("gif encodes");
    }

    assert_eq!(decode_image(&bytes, None).expect("gif decodes"), first);
}

#[test]
fn converts_gray_and_palette_to_rgba() {
    let gray = png_from_rows(2, 0, &[&[100, 200]], &[]);
    let gray_alpha = png_from_rows(2, 4, &[&[100, 50, 200, 255]], &[]);
    let palette = png_from_rows(
        2,
        3,
        &[&[0, 1]],
        &[(b"PLTE", vec![255, 0, 0, 0, 255, 0]), (b"tRNS", vec![128])],
    );

    assert_eq!(
        decode_image(&gray, None).expect("gray decodes").into_raw(),
        [100, 100, 100, 255, 200, 200, 200, 255]
    );
    assert_eq!(
        decode_image(&gray_alpha, None)
            .expect("gray alpha decodes")
            .into_raw(),
        [100, 100, 100, 50, 200, 200, 200, 255]
    );
    assert_eq!(
        decode_image(&palette, None)
            .expect("palette decodes")
            .into_raw(),
        [255, 0, 0, 128, 0, 255, 0, 255]
    );
}

#[test]
fn exif_orientation_is_not_applied() {
    let source = RgbaImage::from_fn(8, 4, |x, y| Rgba([(x * 30) as u8, (y * 60) as u8, 90, 255]));
    let jpeg = encode_jpeg(source, 90);

    // Big-endian TIFF with one IFD entry: Orientation (0x0112), SHORT, count 1, value 6.
    let mut exif = b"Exif\0\0MM\0*\0\0\0\x08".to_vec();
    exif.extend_from_slice(&[0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
    let mut app1 = vec![0xFF, 0xE1];
    app1.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    app1.extend(exif);
    let with_exif = [&jpeg[..2], &app1, &jpeg[2..]].concat();

    let decoded = decode_image(&with_exif, None).expect("jpeg decodes");

    assert_eq!(decoded.dimensions(), (8, 4));
}
