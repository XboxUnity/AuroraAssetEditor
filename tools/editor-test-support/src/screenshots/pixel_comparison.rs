use std::fmt;

use image::RgbaImage;

/// Every pixel of the union of both sizes that differs, row by row. A pixel that exists in only
/// one image differs. This is the one place pixels are compared.
pub fn changed_pixels<'a>(
    reference: &'a RgbaImage,
    actual: &'a RgbaImage,
) -> impl Iterator<Item = (u32, u32)> + 'a {
    let width = reference.width().max(actual.width());
    let height = reference.height().max(actual.height());
    (0..height)
        .flat_map(move |y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| reference.get_pixel_checked(x, y) != actual.get_pixel_checked(x, y))
}

pub fn compare_pixels(reference: &RgbaImage, actual: &RgbaImage) -> PixelComparison {
    if reference.dimensions() != actual.dimensions() {
        return PixelComparison::DifferentSize {
            reference: reference.dimensions(),
            actual: actual.dimensions(),
        };
    }

    let mut difference: Option<PixelDifference> = None;
    for (x, y) in changed_pixels(reference, actual) {
        let channel_difference = largest_channel_difference(reference, actual, x, y);
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

fn largest_channel_difference(reference: &RgbaImage, actual: &RgbaImage, x: u32, y: u32) -> u8 {
    reference
        .get_pixel(x, y)
        .0
        .iter()
        .zip(actual.get_pixel(x, y).0)
        .map(|(expected, found)| expected.abs_diff(found))
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelComparison {
    Same,
    DifferentSize {
        reference: (u32, u32),
        actual: (u32, u32),
    },
    DifferentPixels(PixelDifference),
}

impl fmt::Display for PixelComparison {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Same => write!(formatter, "the images are the same"),
            Self::DifferentSize { reference, actual } => write!(
                formatter,
                "the size changed from {}x{} to {}x{}",
                reference.0, reference.1, actual.0, actual.1
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
            bounds: PixelBounds {
                left: x,
                top: y,
                right: x,
                bottom: y,
            },
            largest_channel_difference: channel_difference,
        }
    }

    fn include(&mut self, x: u32, y: u32, channel_difference: u8) {
        self.count += 1;
        self.bounds.left = self.bounds.left.min(x);
        self.bounds.top = self.bounds.top.min(y);
        self.bounds.right = self.bounds.right.max(x);
        self.bounds.bottom = self.bounds.bottom.max(y);
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
