use aurora_asset::RgbaImage;
use aurora_image::{preview_size, resize_preview};
use image::Rgba;

const BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Black and white pixels in a checkerboard of `square`-pixel squares.
fn checkerboard(side: u32, square: u32) -> RgbaImage {
    RgbaImage::from_fn(side, side, |x, y| {
        if (x / square + y / square).is_multiple_of(2) {
            BLACK
        } else {
            WHITE
        }
    })
}

fn is_grey(pixel: &Rgba<u8>) -> bool {
    pixel[0] > BLACK[0] && pixel[0] < WHITE[0]
}

#[test]
fn preview_size_fits_the_area_and_keeps_the_aspect_ratio() {
    let cases = [
        ((900, 600), (863, 569), (854, 569)),
        ((1000, 562), (62, 33), (59, 33)),
        ((64, 64), (863, 226), (226, 226)),
        ((420, 96), (863, 226), (863, 197)),
        ((300, 200), (300, 200), (300, 200)),
        ((5000, 1), (10, 10), (10, 1)),
        ((10, 10), (0, 0), (1, 1)),
    ];
    for (source, area, expected) in cases {
        assert_eq!(
            preview_size(source, area),
            expected,
            "{source:?} in {area:?}"
        );
    }
}

#[test]
fn an_enlarged_preview_blends_between_pixels() {
    let enlarged = resize_preview(&checkerboard(4, 1), (40, 40));

    assert_eq!(enlarged.dimensions(), (40, 40));
    let blended = enlarged.pixels().filter(|pixel| is_grey(pixel)).count();
    assert!(
        blended > enlarged.pixels().len() / 2,
        "only {blended} pixels blend: the squares were copied as blocks"
    );
}

#[test]
fn a_shrunk_preview_averages_the_pixels_it_covers() {
    let shrunk = resize_preview(&checkerboard(64, 1), (8, 8));

    assert_eq!(shrunk.dimensions(), (8, 8));
    for pixel in shrunk.pixels() {
        assert!(
            pixel[0].abs_diff(128) <= 8,
            "{pixel:?} is not the average grey: single pixels were picked"
        );
    }
}
