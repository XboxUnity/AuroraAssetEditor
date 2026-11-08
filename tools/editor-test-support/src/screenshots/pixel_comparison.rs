use std::fmt;

use image::RgbaImage;

/// Every pixel of the union of both sizes that differs, row by row. A pixel that exists in only
/// one image differs. This is the one place pixels are compared.
pub fn changed_pixels<'a>(
    baseline: &'a RgbaImage,
    current: &'a RgbaImage,
) -> impl Iterator<Item = (u32, u32)> + 'a {
    let width = baseline.width().max(current.width());
    let height = baseline.height().max(current.height());
    (0..height)
        .flat_map(move |y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| baseline.get_pixel_checked(x, y) != current.get_pixel_checked(x, y))
}

pub fn compare_pixels(baseline: &RgbaImage, current: &RgbaImage) -> PixelComparison {
    if baseline.dimensions() != current.dimensions() {
        return PixelComparison::DifferentSize {
            baseline: baseline.dimensions(),
            current: current.dimensions(),
        };
    }

    let mut difference: Option<PixelDifference> = None;
    for (x, y) in changed_pixels(baseline, current) {
        let channel_difference = largest_channel_difference(baseline, current, x, y);
        match difference.as_mut() {
            Some(known) => known.include(x, y, channel_difference),
            None => difference = Some(PixelDifference::first(x, y, channel_difference)),
        }
    }
    match difference {
        Some(difference) => PixelComparison::DifferentPixels(difference),
        None => PixelComparison::Same,
    }
}

fn largest_channel_difference(baseline: &RgbaImage, current: &RgbaImage, x: u32, y: u32) -> u8 {
    baseline
        .get_pixel(x, y)
        .0
        .iter()
        .zip(current.get_pixel(x, y).0)
        .map(|(expected, found)| expected.abs_diff(found))
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelComparison {
    Same,
    DifferentSize {
        baseline: (u32, u32),
        current: (u32, u32),
    },
    DifferentPixels(PixelDifference),
}

impl fmt::Display for PixelComparison {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Same => write!(formatter, "the images are the same"),
            Self::DifferentSize { baseline, current } => write!(
                formatter,
                "the size changed from {}x{} to {}x{}",
                baseline.0, baseline.1, current.0, current.1
            ),
            Self::DifferentPixels(difference) => difference.fmt(formatter),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelDifference {
    pub count: u64,
    pub bounds: PixelBounds,
    pub largest_channel_difference: u8,
}

impl PixelDifference {
    fn first(x: u32, y: u32, channel_difference: u8) -> Self {
        Self {
            count: 1,
            bounds: PixelBounds::around(x, y),
            largest_channel_difference: channel_difference,
        }
    }

    fn include(&mut self, x: u32, y: u32, channel_difference: u8) {
        self.count += 1;
        self.bounds.include(x, y);
        self.largest_channel_difference = self.largest_channel_difference.max(channel_difference);
    }
}

impl fmt::Display for PixelDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bounds = &self.bounds;
        let verb_phrase = match self.count {
            1 => "pixel differs",
            _ => "pixels differ",
        };
        write!(
            formatter,
            "{} {verb_phrase} in x {}..={}, y {}..={} (largest channel difference {})",
            group_thousands(self.count),
            bounds.left,
            bounds.right,
            bounds.top,
            bounds.bottom,
            self.largest_channel_difference
        )
    }
}

/// The bounding box of the changed pixels; every edge is inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelBounds {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl PixelBounds {
    /// The box holding just the pixel (x, y).
    pub fn around(x: u32, y: u32) -> Self {
        Self {
            left: x,
            top: y,
            right: x,
            bottom: y,
        }
    }

    /// Grows the box to hold the pixel (x, y) as well.
    pub fn include(&mut self, x: u32, y: u32) {
        self.left = self.left.min(x);
        self.top = self.top.min(y);
        self.right = self.right.max(x);
        self.bottom = self.bottom.max(y);
    }
}

/// 1234 becomes "1,234".
fn group_thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}
