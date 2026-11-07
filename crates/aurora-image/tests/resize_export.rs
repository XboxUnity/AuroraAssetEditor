use std::path::Path;

use aurora_asset::{AssetSlot, RgbaImage};
use aurora_image::{ExportFormat, ImageTarget, Resampler, decode_image, fit_to_target};
use image::Rgba;

fn gradient(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        Rgba([
            (x * 7 % 256) as u8,
            (y * 13 % 256) as u8,
            ((x + y) % 256) as u8,
            255,
        ])
    })
}

fn with_transparent_pixels() -> RgbaImage {
    RgbaImage::from_fn(8, 8, |x, y| {
        if (x + y) % 2 == 0 {
            Rgba([200, 100, 50, 0])
        } else {
            Rgba([10, 20, 30, 255])
        }
    })
}

#[test]
fn targets_have_aurora_sizes() {
    assert_eq!(ImageTarget::Cover.size(), (900, 600));
    assert_eq!(ImageTarget::Background.size(), (1280, 720));
    assert_eq!(ImageTarget::Icon.size(), (64, 64));
    assert_eq!(ImageTarget::Banner.size(), (420, 96));
    assert_eq!(ImageTarget::Screenshot.size(), (1000, 562));
}

#[test]
fn targets_map_to_slots() {
    assert_eq!(
        ImageTarget::for_slot(AssetSlot::Cover),
        Some(ImageTarget::Cover)
    );
    assert_eq!(
        ImageTarget::for_slot(AssetSlot::Background),
        Some(ImageTarget::Background)
    );
    assert_eq!(
        ImageTarget::for_slot(AssetSlot::Icon),
        Some(ImageTarget::Icon)
    );
    assert_eq!(
        ImageTarget::for_slot(AssetSlot::Banner),
        Some(ImageTarget::Banner)
    );
    let screenshot = AssetSlot::screenshot(7).expect("7 is a valid screenshot number");
    assert_eq!(
        ImageTarget::for_slot(screenshot),
        Some(ImageTarget::Screenshot)
    );
    assert_eq!(ImageTarget::for_slot(AssetSlot::Slot), None);
}

#[test]
fn stretches_to_target_ignoring_aspect() {
    let stretched = fit_to_target(gradient(100, 50), ImageTarget::Cover, true);
    assert_eq!(stretched.dimensions(), (900, 600));
}

#[test]
fn every_target_is_reached_exactly() {
    for target in [
        ImageTarget::Cover,
        ImageTarget::Background,
        ImageTarget::Icon,
        ImageTarget::Banner,
        ImageTarget::Screenshot,
    ] {
        let stretched = fit_to_target(gradient(33, 17), target, true);
        assert_eq!(stretched.dimensions(), target.size());
    }
}

#[test]
fn target_size_passes_through_untouched() {
    let icon = gradient(64, 64);
    let result = fit_to_target(icon.clone(), ImageTarget::Icon, true);
    assert_eq!(result.as_raw(), icon.as_raw());
}

#[test]
fn auto_resize_off_keeps_size() {
    let image = gradient(100, 50);
    let result = fit_to_target(image.clone(), ImageTarget::Cover, false);
    assert_eq!(result.dimensions(), (100, 50));
    assert_eq!(result.as_raw(), image.as_raw());
}

#[test]
fn every_resampler_reaches_the_requested_size() {
    for resampler in Resampler::ALL {
        let resized = resampler.resize(&with_transparent_pixels(), 20, 12);
        assert_eq!(resized.dimensions(), (20, 12), "{}", resampler.name());
    }
}

#[test]
fn premultiplied_resize_ignores_colour_of_transparent_pixels() {
    let mut other_colour = with_transparent_pixels();
    other_colour
        .pixels_mut()
        .filter(|pixel| pixel[3] == 0)
        .for_each(|pixel| *pixel = Rgba([0, 255, 0, 0]));

    let plain = Resampler::Triangle.resize(&with_transparent_pixels(), 4, 4);
    let plain_other = Resampler::Triangle.resize(&other_colour, 4, 4);
    assert_ne!(plain.as_raw(), plain_other.as_raw());

    let premultiplied = Resampler::TrianglePremultiplied.resize(&with_transparent_pixels(), 4, 4);
    let premultiplied_other = Resampler::TrianglePremultiplied.resize(&other_colour, 4, 4);
    assert_eq!(premultiplied.as_raw(), premultiplied_other.as_raw());
}

#[test]
fn export_format_follows_extension() {
    let cases = [
        ("a.PNG", ExportFormat::Png),
        ("a.jpeg", ExportFormat::Jpeg),
        ("a.JPG", ExportFormat::Jpeg),
        ("a.tif", ExportFormat::Tiff),
        ("a.tiff", ExportFormat::Tiff),
        ("a.webp", ExportFormat::WebP),
        ("a.gif", ExportFormat::Gif),
        ("a.bmp", ExportFormat::Bmp),
        ("cover", ExportFormat::Png),
        ("a.dds", ExportFormat::Png),
    ];
    for (path, expected) in cases {
        assert_eq!(ExportFormat::from_path(Path::new(path)), expected, "{path}");
    }
}

#[test]
fn png_export_round_trips() {
    let image = with_transparent_pixels();
    let bytes = ExportFormat::Png.encode(&image).expect("png encodes");
    let decoded = decode_image(&bytes, None).expect("png decodes");
    assert_eq!(decoded.as_raw(), image.as_raw());
}

#[test]
fn webp_export_is_lossless() {
    let image = gradient(31, 17);
    let bytes = ExportFormat::WebP.encode(&image).expect("webp encodes");
    let decoded = decode_image(&bytes, None).expect("webp decodes");
    assert_eq!(decoded.as_raw(), image.as_raw());
}

#[test]
fn bmp_tiff_gif_exports_decode() {
    let image = gradient(16, 9);
    for format in [ExportFormat::Bmp, ExportFormat::Tiff, ExportFormat::Gif] {
        let bytes = format.encode(&image).expect("encodes");
        let decoded = decode_image(&bytes, None).expect("decodes");
        assert_eq!(decoded.dimensions(), (16, 9), "{format:?}");
    }
    for format in [ExportFormat::Bmp, ExportFormat::Tiff] {
        let bytes = format.encode(&image).expect("encodes");
        let decoded = decode_image(&bytes, None).expect("decodes");
        assert_eq!(decoded.as_raw(), image.as_raw(), "{format:?}");
    }
}

#[test]
fn jpeg_export_drops_alpha() {
    let image = RgbaImage::from_pixel(16, 16, Rgba([200, 40, 40, 0]));
    let bytes = ExportFormat::Jpeg.encode(&image).expect("jpeg encodes");
    let decoded = decode_image(&bytes, None).expect("jpeg decodes");
    for pixel in decoded.pixels() {
        assert_eq!(pixel[3], 255);
        assert!(pixel[0].abs_diff(200) <= 6, "red {}", pixel[0]);
        assert!(pixel[1].abs_diff(40) <= 6, "green {}", pixel[1]);
    }
}
