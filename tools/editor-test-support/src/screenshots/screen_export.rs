use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use image::{ImageError, RgbaImage};
use oxipng::Options;

use super::capture_thread::capture_on_own_thread;
use super::image_files::{png_path, validate_image_name};
use super::png_file::{PngFileError, opaque_png_bytes, read_png, write_replacing};
use super::screen::{SCREEN_THEMES, Screen, image_name};

/// Writes screens into a folder of committed images, such as the README's, each optimised
/// losslessly. A file whose pixels equal the new capture keeps its bytes, so re-exporting an
/// unchanged screen changes nothing in git. Files of the folder that are not images of these
/// screens are left alone.
pub struct ScreenExport {
    folder: PathBuf,
}

#[derive(Debug, Default)]
pub struct ExportReport {
    pub written: Vec<String>,
    /// Images whose file already held the same pixels.
    pub kept: Vec<String>,
    /// One message per image that could not be exported, naming the image.
    pub errors: Vec<String>,
}

enum ExportOutcome {
    Written,
    Kept,
}

impl ScreenExport {
    pub fn new(folder: PathBuf) -> ScreenExport {
        ScreenExport { folder }
    }

    /// Every screen in both themes, each on its own thread. An image that fails is reported and
    /// the others still go.
    pub fn export(&self, screens: &[Screen]) -> ExportReport {
        let mut report = ExportReport::default();
        for screen in screens {
            for (theme, theme_name) in SCREEN_THEMES {
                let image = image_name(screen, theme_name);
                let outcome = validate_image_name(&image)
                    .map_err(|error| error.to_string())
                    .and_then(|()| capture_on_own_thread(&image, screen.stage, theme))
                    .and_then(|capture| self.store(&image, &capture));
                match outcome {
                    Ok(ExportOutcome::Written) => report.written.push(image),
                    Ok(ExportOutcome::Kept) => report.kept.push(image),
                    Err(message) => report.errors.push(format!("{image}: {message}")),
                }
            }
        }
        report
    }

    fn store(&self, image: &str, capture: &RgbaImage) -> Result<ExportOutcome, String> {
        let path = png_path(&self.folder, image);
        if holds_pixels(&path, capture)? {
            return Ok(ExportOutcome::Kept);
        }
        let png = optimised_png(capture)?;
        write_replacing(&path, &png)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        Ok(ExportOutcome::Written)
    }
}

/// `capture` as the suite encodes it, then made smaller by oxipng with the same pixels. Its highest
/// preset: the images are committed and rarely rewritten, so the smallest file is worth the time.
fn optimised_png(capture: &RgbaImage) -> Result<Vec<u8>, String> {
    let png = opaque_png_bytes(capture).map_err(|error| error.to_string())?;
    oxipng::optimize_from_memory(&png, &Options::max_compression())
        .map_err(|error| format!("could not optimise the PNG: {error}"))
}

/// Whether `path` holds a PNG of exactly `capture`. A file that is not there, or is not a PNG
/// (including an empty or cut-off one), holds nothing and gets replaced; any other failure to read
/// it is an error.
fn holds_pixels(path: &Path, capture: &RgbaImage) -> Result<bool, String> {
    match read_png(path) {
        Ok(existing) => Ok(existing == *capture),
        Err(PngFileError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        // The decoder reports both a file that ends early and a failed read as an I/O error; only
        // the first means the file is not a PNG.
        Err(PngFileError::Decode(ImageError::IoError(error)))
            if error.kind() == io::ErrorKind::UnexpectedEof =>
        {
            Ok(false)
        }
        Err(PngFileError::Decode(ImageError::IoError(error))) => Err(unreadable(path, &error)),
        Err(PngFileError::Decode(_)) => Ok(false),
        Err(error) => Err(unreadable(path, &error)),
    }
}

fn unreadable(path: &Path, error: &dyn fmt::Display) -> String {
    format!("could not read {}: {error}", path.display())
}

impl ExportReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for ExportReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Written: {}", listed(&self.written))?;
        writeln!(formatter, "Kept (pixels unchanged): {}", listed(&self.kept))?;
        if self.errors.is_empty() {
            return writeln!(formatter, "Errors: none");
        }
        writeln!(formatter, "Errors:")?;
        for error in &self.errors {
            writeln!(formatter, "  {error}")?;
        }
        Ok(())
    }
}

fn listed(images: &[String]) -> String {
    if images.is_empty() {
        return "none".to_owned();
    }
    images.join(", ")
}
