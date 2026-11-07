use editor_test_support::{
    DIFF_BOUNDS_COLOUR, DIFF_CHANGED_COLOUR, DIFF_GAP_COLOUR, DIFF_PANEL_GAP, PixelBounds,
    PixelComparison, PixelDifference, PngFileError, changed_pixels, compare_pixels, diff_image,
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

fn dimmed_grey(pixel: Rgba<u8>) -> Rgba<u8> {
    let [red, green, blue, _] = pixel.0;
    let grey = (299 * u32::from(red) + 587 * u32::from(green) + 114 * u32::from(blue)) / 1000;
    let level = u8::try_from(grey * 30 / 100).expect("a dimmed channel fits in a byte");
    Rgba([level, level, level, 255])
}

fn image_with_changed_pixels(base: &RgbaImage, changed: &[(u32, u32)]) -> RgbaImage {
    let mut image = base.clone();
    for &(x, y) in changed {
        image.put_pixel(x, y, Rgba([200, 100, 50, 255]));
    }
    image
}

#[test]
fn diff_image_shows_reference_actual_and_changes_side_by_side() {
    let reference = plain_image(10, 6);
    let actual = image_with_changed_pixels(&reference, &[(3, 2)]);

    let diff = diff_image(&reference, &actual);

    assert_eq!(diff.dimensions(), (3 * 10 + 2 * DIFF_PANEL_GAP, 6));
    let panel_step = 10 + DIFF_PANEL_GAP;
    assert_eq!(*diff.get_pixel(0, 0), *reference.get_pixel(0, 0));
    assert_eq!(*diff.get_pixel(3, 2), *reference.get_pixel(3, 2));
    assert_eq!(*diff.get_pixel(panel_step + 3, 2), *actual.get_pixel(3, 2));
    assert_eq!(*diff.get_pixel(panel_step + 9, 5), *actual.get_pixel(9, 5));
    for gap_start in [10, 2 * 10 + DIFF_PANEL_GAP] {
        for x in gap_start..gap_start + DIFF_PANEL_GAP {
            assert_eq!(*diff.get_pixel(x, 3), DIFF_GAP_COLOUR);
        }
    }
}

#[test]
fn changed_pixels_are_magenta_on_a_dimmed_copy() {
    let reference = plain_image(10, 6);
    let actual = image_with_changed_pixels(&reference, &[(3, 2)]);

    let diff = diff_image(&reference, &actual);

    let changes_left = 2 * (10 + DIFF_PANEL_GAP);
    assert_eq!(*diff.get_pixel(changes_left + 3, 2), DIFF_CHANGED_COLOUR);
    assert_eq!(
        *diff.get_pixel(changes_left + 9, 5),
        dimmed_grey(*actual.get_pixel(9, 5))
    );
}

#[test]
fn difference_bounds_are_outlined_on_every_panel() {
    let reference = plain_image(10, 8);
    let actual = image_with_changed_pixels(&reference, &[(3, 2), (6, 4)]);

    let diff = diff_image(&reference, &actual);

    let panel_step = 10 + DIFF_PANEL_GAP;
    for panel in 0..3 {
        let left = panel * panel_step;
        for x in 2..=7 {
            assert_eq!(*diff.get_pixel(left + x, 1), DIFF_BOUNDS_COLOUR);
            assert_eq!(*diff.get_pixel(left + x, 5), DIFF_BOUNDS_COLOUR);
        }
        for y in 1..=5 {
            assert_eq!(*diff.get_pixel(left + 2, y), DIFF_BOUNDS_COLOUR);
            assert_eq!(*diff.get_pixel(left + 7, y), DIFF_BOUNDS_COLOUR);
        }
    }
    let changes_left = 2 * panel_step;
    assert_eq!(*diff.get_pixel(changes_left + 3, 2), DIFF_CHANGED_COLOUR);
    assert_eq!(*diff.get_pixel(changes_left + 6, 4), DIFF_CHANGED_COLOUR);
}

#[test]
fn an_outline_at_the_image_edge_is_clipped_to_its_panel() {
    let reference = plain_image(4, 4);
    let actual = image_with_changed_pixels(&reference, &[(0, 0)]);

    let diff = diff_image(&reference, &actual);

    let panel_step = 4 + DIFF_PANEL_GAP;
    assert_eq!(*diff.get_pixel(1, 1), DIFF_BOUNDS_COLOUR);
    assert_eq!(*diff.get_pixel(0, 1), DIFF_BOUNDS_COLOUR);
    assert_eq!(*diff.get_pixel(1, 0), DIFF_BOUNDS_COLOUR);
    assert_eq!(*diff.get_pixel(panel_step + 1, 1), DIFF_BOUNDS_COLOUR);
    assert_eq!(*diff.get_pixel(4, 0), DIFF_GAP_COLOUR);
}

#[test]
fn diff_of_different_sizes_keeps_both_whole() {
    let reference = plain_image(4, 4);
    let actual = plain_image(6, 3);

    let diff = diff_image(&reference, &actual);

    assert_eq!(diff.dimensions(), (3 * 6 + 2 * DIFF_PANEL_GAP, 4));
    let panel_step = 6 + DIFF_PANEL_GAP;
    assert_eq!(*diff.get_pixel(5, 0), DIFF_GAP_COLOUR);
    assert_eq!(*diff.get_pixel(0, 3), *reference.get_pixel(0, 3));
    assert_eq!(*diff.get_pixel(panel_step, 3), DIFF_GAP_COLOUR);
    assert_eq!(*diff.get_pixel(panel_step + 5, 2), *actual.get_pixel(5, 2));
    let changes_left = 2 * panel_step;
    assert_eq!(*diff.get_pixel(changes_left + 5, 1), DIFF_CHANGED_COLOUR);
    assert_eq!(*diff.get_pixel(changes_left, 3), DIFF_CHANGED_COLOUR);
}
