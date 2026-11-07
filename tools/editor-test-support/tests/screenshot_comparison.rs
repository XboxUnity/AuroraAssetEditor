use editor_test_support::{
    PixelBounds, PixelComparison, PixelDifference, PngFileError, changed_pixels, compare_pixels,
    read_png, write_opaque_png,
};
use image::{ColorType, ImageReader, Rgba, RgbaImage};

fn plain_image(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]))
}

fn bounds(left: u32, top: u32, right: u32, bottom: u32) -> PixelBounds {
    PixelBounds {
        left,
        top,
        right,
        bottom,
    }
}

#[test]
fn opaque_png_round_trips_pixels() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the PNG");
    let path = folder.path().join("nested").join("image.png");
    let mut image = RgbaImage::new(3, 2);
    for (index, pixel) in image.pixels_mut().enumerate() {
        let step = index as u8 * 40;
        *pixel = Rgba([step, 255 - step, step / 2, 255]);
    }

    write_opaque_png(&path, &image).expect("an opaque image is written");

    assert_eq!(read_png(&path).expect("the PNG reads back"), image);
    let color = ImageReader::open(&path)
        .expect("the PNG opens")
        .decode()
        .expect("the PNG decodes")
        .color();
    assert_eq!(color, ColorType::Rgb8);
}

#[test]
fn translucent_image_is_refused() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the PNG");
    let path = folder.path().join("image.png");
    let mut image = plain_image(3, 2);
    image.put_pixel(1, 0, Rgba([10, 20, 30, 254]));

    let error = write_opaque_png(&path, &image).expect_err("a translucent pixel is refused");

    assert!(matches!(error, PngFileError::NotOpaque { x: 1, y: 0 }));
    assert!(!path.exists());
}

#[test]
fn a_refused_write_leaves_an_existing_file_untouched() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the PNG");
    let path = folder.path().join("image.png");
    std::fs::write(&path, b"existing bytes").expect("the existing file is created");
    let mut image = plain_image(3, 2);
    image.put_pixel(2, 1, Rgba([10, 20, 30, 0]));

    write_opaque_png(&path, &image).expect_err("a translucent pixel is refused");

    assert_eq!(
        std::fs::read(&path).expect("the existing file is still there"),
        b"existing bytes"
    );
    assert_eq!(
        std::fs::read_dir(folder.path())
            .expect("the folder lists")
            .count(),
        1
    );
}

#[test]
fn a_write_replaces_an_existing_file_and_leaves_no_temporary_file() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the PNG");
    let path = folder.path().join("image.png");
    std::fs::write(&path, b"old bytes").expect("the existing file is created");
    let image = plain_image(3, 2);

    write_opaque_png(&path, &image).expect("an opaque image is written");

    assert_eq!(read_png(&path).expect("the PNG reads back"), image);
    assert_eq!(
        std::fs::read_dir(folder.path())
            .expect("the folder lists")
            .count(),
        1
    );
}

#[test]
fn a_file_that_is_not_a_png_fails_to_decode() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed for the file");
    let path = folder.path().join("image.png");
    std::fs::write(&path, b"this is not a PNG").expect("the corrupt file is created");

    let error = read_png(&path).expect_err("a corrupt file is refused");

    assert!(matches!(error, PngFileError::Decode(_)));
    assert!(error.to_string().starts_with("the file is not a valid PNG"));
}

#[test]
fn identical_images_are_the_same() {
    let image = plain_image(8, 8);

    assert_eq!(
        compare_pixels(&image, &image.clone()),
        PixelComparison::Same
    );
}

#[test]
fn one_changed_channel_is_a_difference() {
    let reference = plain_image(16, 16);
    let mut actual = reference.clone();
    actual.put_pixel(5, 7, Rgba([10, 21, 30, 255]));

    assert_eq!(
        compare_pixels(&reference, &actual),
        PixelComparison::DifferentPixels(PixelDifference {
            count: 1,
            bounds: bounds(5, 7, 5, 7),
            largest_channel_difference: 1,
        })
    );
}

#[test]
fn every_channel_counts() {
    let reference = plain_image(4, 4);
    let mut actual = reference.clone();
    actual.put_pixel(0, 0, Rgba([10, 20, 30, 254]));

    let comparison = compare_pixels(&reference, &actual);

    assert!(matches!(
        comparison,
        PixelComparison::DifferentPixels(PixelDifference { count: 1, .. })
    ));
}

#[test]
fn difference_bounds_cover_every_changed_pixel() {
    let reference = plain_image(16, 16);
    let mut actual = reference.clone();
    actual.put_pixel(2, 3, Rgba([50, 20, 30, 255]));
    actual.put_pixel(9, 1, Rgba([10, 25, 30, 255]));

    assert_eq!(
        compare_pixels(&reference, &actual),
        PixelComparison::DifferentPixels(PixelDifference {
            count: 2,
            bounds: bounds(2, 1, 9, 3),
            largest_channel_difference: 40,
        })
    );
}

#[test]
fn different_sizes_are_a_difference() {
    assert_eq!(
        compare_pixels(&plain_image(4, 4), &plain_image(4, 5)),
        PixelComparison::DifferentSize {
            reference: (4, 4),
            actual: (4, 5),
        }
    );
}

#[test]
fn a_pixel_in_only_one_image_is_changed() {
    let reference = plain_image(4, 4);
    let actual = plain_image(6, 3);

    let changed: Vec<(u32, u32)> = changed_pixels(&reference, &actual).collect();

    // Right strip (x 4..6, y 0..3) exists only in the actual, bottom row (x 0..4, y 3) only in the
    // reference. The corner (x 4..6, y 3) is in neither image, so it is not a difference.
    let mut expected: Vec<(u32, u32)> = Vec::new();
    for y in 0..4 {
        for x in 0..6 {
            let only_in_actual = x >= 4 && y < 3;
            let only_in_reference = x < 4 && y >= 3;
            if only_in_actual || only_in_reference {
                expected.push((x, y));
            }
        }
    }
    assert_eq!(changed.len(), 10);
    assert_eq!(changed, expected);
    assert!(changed.contains(&(4, 0)));
    assert!(changed.contains(&(0, 3)));
}

#[test]
fn comparison_reads_as_one_sentence() {
    let reference = plain_image(16, 16);
    let mut two_changes = reference.clone();
    two_changes.put_pixel(2, 3, Rgba([50, 20, 30, 255]));
    two_changes.put_pixel(9, 1, Rgba([10, 25, 30, 255]));
    assert_eq!(
        compare_pixels(&reference, &two_changes).to_string(),
        "2 pixels differ in x 2..=9, y 1..=3 (largest channel difference 40)"
    );

    assert_eq!(
        compare_pixels(&plain_image(4, 4), &plain_image(4, 5)).to_string(),
        "the size changed from 4x4 to 4x5"
    );

    let mut one_change = reference.clone();
    one_change.put_pixel(5, 7, Rgba([10, 21, 30, 255]));
    assert_eq!(
        compare_pixels(&reference, &one_change).to_string(),
        "1 pixel differs in x 5..=5, y 7..=7 (largest channel difference 1)"
    );

    let many = PixelComparison::DifferentPixels(PixelDifference {
        count: 1234,
        bounds: bounds(0, 0, 9, 9),
        largest_channel_difference: 7,
    });
    assert_eq!(
        many.to_string(),
        "1,234 pixels differ in x 0..=9, y 0..=9 (largest channel difference 7)"
    );
}
