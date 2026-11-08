use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::RgbaImage;

use super::baseline::Baseline;
use super::diff_image::diff_image;
use super::image_change::ImageChange;
use super::image_files::{png_path, validate_image_name};
use super::pixel_comparison::{PixelComparison, compare_pixels};
use super::png_file::{PngFileError, first_translucent_pixel, read_png, write_opaque_png};

pub const IMAGES_FOLDER: &str = "images";
pub const BASELINE_COPY_FOLDER: &str = "baseline";
pub const DIFF_FOLDER: &str = "diff";

/// The platform this run renders, as `{OS}-{ARCH}`, e.g. `windows-x86_64`.
pub fn current_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The folder one run writes into: `images/<image>.png` for every image, and for an image that
/// changed also `baseline/<image>.png` (the baseline's file) and `diff/<image>.png`; for a removed
/// image only `baseline/<image>.png`.
#[derive(Debug, Clone)]
pub struct RunFolder {
    folder: PathBuf,
}

impl RunFolder {
    pub fn new(folder: PathBuf) -> RunFolder {
        RunFolder { folder }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn image_path(&self, image: &str) -> PathBuf {
        png_path(&self.folder.join(IMAGES_FOLDER), image)
    }

    pub fn baseline_copy_path(&self, image: &str) -> PathBuf {
        png_path(&self.folder.join(BASELINE_COPY_FOLDER), image)
    }

    pub fn diff_path(&self, image: &str) -> PathBuf {
        png_path(&self.folder.join(DIFF_FOLDER), image)
    }

    /// Writes `current` and compares it with the baseline's image of the same name.
    pub fn record(
        &self,
        image: &str,
        current: &RgbaImage,
        baseline: Option<&Baseline>,
    ) -> ImageChange {
        match self.try_record(image, current, baseline) {
            Ok(change) => change,
            Err(message) => ImageChange::Error(message),
        }
    }

    /// Copies the baseline's file of an image the current run no longer renders.
    pub fn keep_removed(&self, image: &str, baseline: &Baseline) -> io::Result<()> {
        validate_image_name(image)?;
        copy_file(&baseline.image_path(image), &self.baseline_copy_path(image))
    }

    fn try_record(
        &self,
        image: &str,
        current: &RgbaImage,
        baseline: Option<&Baseline>,
    ) -> Result<ImageChange, String> {
        validate_image_name(image).map_err(|error| error.to_string())?;

        let image_path = self.image_path(image);
        write_opaque_png(&image_path, current)
            .map_err(|error| failure("write the screenshot", &image_path, error))?;

        let Some(baseline) = baseline else {
            return Ok(ImageChange::New);
        };
        let baseline_path = baseline.image_path(image);
        let baseline_image = match read_png(&baseline_path) {
            Ok(baseline_image) => baseline_image,
            Err(PngFileError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ImageChange::New);
            }
            Err(error) => return Ok(unreadable_baseline(&baseline_path, error)),
        };
        if let Some((x, y)) = first_translucent_pixel(&baseline_image) {
            let error = PngFileError::NotOpaque { x, y };
            return Ok(unreadable_baseline(&baseline_path, error));
        }

        let comparison = compare_pixels(&baseline_image, current);
        if comparison == PixelComparison::Same {
            return Ok(ImageChange::Unchanged);
        }

        copy_file(&baseline_path, &self.baseline_copy_path(image))
            .map_err(|error| error.to_string())?;
        let diff_path = self.diff_path(image);
        write_opaque_png(&diff_path, &diff_image(&baseline_image, current))
            .map_err(|error| failure("write the diff", &diff_path, error))?;
        Ok(ImageChange::Changed(comparison))
    }
}

fn unreadable_baseline(path: &Path, error: PngFileError) -> ImageChange {
    ImageChange::BaselineUnreadable(format!(
        "Could not use the baseline image {}: {error}",
        path.display()
    ))
}

fn failure(action: &str, path: &Path, error: impl Display) -> String {
    format!("Could not {action} {}: {error}", path.display())
}

/// Copies the baseline's file `source` to `target`, creating the folder of `target`. An error
/// says what was being copied and names both paths, and keeps its kind.
fn copy_file(source: &Path, target: &Path) -> io::Result<()> {
    if let Some(folder) = target.parent() {
        fs::create_dir_all(folder).map_err(|error| with_path(folder, error))?;
    }
    fs::copy(source, target).map(|_| ()).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Could not copy the baseline image {} to {}: {error}",
                source.display(),
                target.display()
            ),
        )
    })
}

fn with_path(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}
