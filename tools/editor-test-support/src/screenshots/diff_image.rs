use image::{Rgba, RgbaImage};

use super::pixel_comparison::{PixelBounds, changed_pixels};

pub const DIFF_PANEL_GAP: u32 = 8;
pub const DIFF_GAP_COLOUR: Rgba<u8> = Rgba([0x80, 0x80, 0x80, 0xff]);
pub const DIFF_CHANGED_COLOUR: Rgba<u8> = Rgba([0xff, 0x00, 0xff, 0xff]);
pub const DIFF_BOUNDS_COLOUR: Rgba<u8> = Rgba([0xff, 0xd4, 0x00, 0xff]);

const PANEL_COUNT: u32 = 3;
const UNCHANGED_BRIGHTNESS_PERCENT: u32 = 30;

/// Three panels side by side: the baseline, the current image, and the changes. The changes panel
/// shows the current image dimmed with every differing pixel in magenta. A yellow outline just
/// outside the bounds of all differences marks the same area on every panel.
pub fn diff_image(baseline: &RgbaImage, current: &RgbaImage) -> RgbaImage {
    let panel_width = baseline.width().max(current.width());
    let panel_height = baseline.height().max(current.height());
    let width = PANEL_COUNT * panel_width + (PANEL_COUNT - 1) * DIFF_PANEL_GAP;
    let mut diff = RgbaImage::from_pixel(width, panel_height, DIFF_GAP_COLOUR);

    draw_panel(&mut diff, panel_left(0, panel_width), baseline, |pixel| {
        pixel
    });
    draw_panel(&mut diff, panel_left(1, panel_width), current, |pixel| {
        pixel
    });
    draw_panel(&mut diff, panel_left(2, panel_width), current, dimmed);

    let mut bounds: Option<PixelBounds> = None;
    for (x, y) in changed_pixels(baseline, current) {
        diff.put_pixel(panel_left(2, panel_width) + x, y, DIFF_CHANGED_COLOUR);
        match bounds.as_mut() {
            Some(known) => known.include(x, y),
            None => bounds = Some(PixelBounds::around(x, y)),
        }
    }

    if let Some(bounds) = bounds {
        for panel in 0..PANEL_COUNT {
            outline_bounds(
                &mut diff,
                panel_left(panel, panel_width),
                panel_width,
                &bounds,
            );
        }
    }
    diff
}

fn dimmed(pixel: Rgba<u8>) -> Rgba<u8> {
    let [red, green, blue, _] = pixel.0;
    let grey = (299 * u32::from(red) + 587 * u32::from(green) + 114 * u32::from(blue)) / 1000;
    let level = u8::try_from(grey * UNCHANGED_BRIGHTNESS_PERCENT / 100)
        .expect("30 percent of a channel value fits in a byte");
    Rgba([level, level, level, 0xff])
}

fn draw_panel(
    diff: &mut RgbaImage,
    left: u32,
    image: &RgbaImage,
    colour_of: impl Fn(Rgba<u8>) -> Rgba<u8>,
) {
    for (x, y, pixel) in image.enumerate_pixels() {
        diff.put_pixel(left + x, y, colour_of(*pixel));
    }
}

/// Draws a one pixel rectangle just outside `bounds`, clipped to the panel starting at `left`.
fn outline_bounds(diff: &mut RgbaImage, left: u32, panel_width: u32, bounds: &PixelBounds) {
    let first_x = bounds.left.checked_sub(1);
    let first_y = bounds.top.checked_sub(1);
    let last_x = bounds.right + 1;
    let last_y = bounds.bottom + 1;
    let panel_height = diff.height();

    let mut put = |x: u32, y: u32| {
        if x < panel_width && y < panel_height {
            diff.put_pixel(left + x, y, DIFF_BOUNDS_COLOUR);
        }
    };
    for x in first_x.unwrap_or(bounds.left)..=last_x {
        if let Some(y) = first_y {
            put(x, y);
        }
        put(x, last_y);
    }
    for y in first_y.unwrap_or(bounds.top)..=last_y {
        if let Some(x) = first_x {
            put(x, y);
        }
        put(last_x, y);
    }
}

/// The left edge of panel `index` when every panel is `panel_width` wide.
fn panel_left(index: u32, panel_width: u32) -> u32 {
    index * (panel_width + DIFF_PANEL_GAP)
}
