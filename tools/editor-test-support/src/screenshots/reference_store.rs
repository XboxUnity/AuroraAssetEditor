use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::RgbaImage;

use super::diff_image::diff_image;
use super::pixel_comparison::{PixelComparison, compare_pixels};
use super::png_file::{PngFileError, first_translucent_pixel, read_png, write_opaque_png};
use super::reference_mode::ReferenceMode;

const UNUSABLE_REFERENCE_REMEDY: &str =
    "delete it and run `cargo xtask ui-screenshots update` to recreate it";
pub(super) const PNG_EXTENSION: &str = "png";
pub(super) const ACTUAL_FOLDER: &str = "actual";
pub(super) const DIFF_FOLDER: &str = "diff";
pub(super) const REFERENCE_COPY_FOLDER: &str = "reference";
const OUTPUT_FOLDERS: [&str; 3] = [ACTUAL_FOLDER, DIFF_FOLDER, REFERENCE_COPY_FOLDER];

/// The approved reference images and the folder one run writes its results into.
///
/// Output layout: `<output>/actual/<image>.png` for every image checked, and for an image that
/// differs also `<output>/reference/<image>.png` (a copy of the reference as it was) and
/// `<output>/diff/<image>.png`.
#[derive(Debug, Clone)]
pub struct ReferenceStore {
    references: PathBuf,
    output: PathBuf,
}

impl ReferenceStore {
    pub fn new(references: PathBuf, output: PathBuf) -> ReferenceStore {
        ReferenceStore { references, output }
    }

    /// `fixtures/ui` and `target/ui-screenshots` of the workspace this crate belongs to.
    pub fn for_workspace() -> ReferenceStore {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("this crate lives two folders below the workspace root");
        ReferenceStore::new(
            workspace.join("fixtures").join("ui"),
            workspace.join("target").join("ui-screenshots"),
        )
    }

    pub fn references(&self) -> &Path {
        &self.references
    }

    pub fn output(&self) -> &Path {
        &self.output
    }

    pub fn reference_path(&self, image: &str) -> PathBuf {
        png_path(&self.references, image)
    }

    pub fn actual_path(&self, image: &str) -> PathBuf {
        png_path(&self.output.join(ACTUAL_FOLDER), image)
    }

    pub fn diff_path(&self, image: &str) -> PathBuf {
        png_path(&self.output.join(DIFF_FOLDER), image)
    }

    pub fn reference_copy_path(&self, image: &str) -> PathBuf {
        png_path(&self.output.join(REFERENCE_COPY_FOLDER), image)
    }

    /// The names (file stems) of all reference PNGs; empty when the folder does not exist yet.
    pub fn reference_names(&self) -> io::Result<BTreeSet<String>> {
        png_names(&self.references)
    }

    /// The names of every image an earlier run left an output for, in any of the output folders.
    pub fn output_names(&self) -> io::Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        for folder in OUTPUT_FOLDERS {
            names.extend(png_names(&self.output.join(folder))?);
        }
        Ok(names)
    }

    pub fn remove_reference(&self, image: &str) -> io::Result<()> {
        validate_image_name(image)?;
        remove_file_with_path(&self.reference_path(image))
    }

    /// Removes what an earlier run left for this image, so an old diff can never describe the
    /// current result. Files that are not there are fine.
    pub fn clear_outputs(&self, image: &str) -> io::Result<()> {
        validate_image_name(image)?;
        let outputs = [
            self.actual_path(image),
            self.diff_path(image),
            self.reference_copy_path(image),
        ];
        for path in outputs {
            match remove_file_with_path(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                other => other?,
            }
        }
        Ok(())
    }

    /// Compares `actual` with the reference of `image`. Writes the actual always; when the pixels
    /// differ also the diff and a copy of the reference. In update mode the reference is replaced
    /// last, after everything meant for review is written, so a failure leaves it untouched.
    pub fn check(&self, image: &str, actual: &RgbaImage, mode: ReferenceMode) -> ImageStatus {
        match self.try_check(image, actual, mode) {
            Ok(status) => status,
            Err(message) => ImageStatus::Error(message),
        }
    }

    fn try_check(
        &self,
        image: &str,
        actual: &RgbaImage,
        mode: ReferenceMode,
    ) -> Result<ImageStatus, String> {
        validate_image_name(image).map_err(|error| error.to_string())?;
        self.clear_outputs(image).map_err(|error| {
            format!("The earlier outputs of {image} could not be removed: {error}")
        })?;

        let actual_path = self.actual_path(image);
        write_opaque_png(&actual_path, actual)
            .map_err(|error| failure("write the screenshot", &actual_path, error))?;

        let reference_path = self.reference_path(image);
        let reference = match read_png(&reference_path) {
            Ok(reference) => reference,
            Err(PngFileError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                return self.missing_reference(image, actual, mode);
            }
            Err(error @ PngFileError::Decode(_)) => {
                return Err(unusable_reference(&reference_path, error));
            }
            Err(error) => return Err(failure("read the reference", &reference_path, error)),
        };
        if let Some((x, y)) = first_translucent_pixel(&reference) {
            let error = PngFileError::NotOpaque { x, y };
            return Err(unusable_reference(&reference_path, error));
        }

        let comparison = compare_pixels(&reference, actual);
        if comparison == PixelComparison::Same {
            return Ok(ImageStatus::Matches);
        }

        let copy_path = self.reference_copy_path(image);
        if let Some(folder) = copy_path.parent() {
            fs::create_dir_all(folder)
                .map_err(|error| failure("create the folder of", &copy_path, error))?;
        }
        fs::copy(&reference_path, &copy_path)
            .map_err(|error| failure("copy the reference to", &copy_path, error))?;
        let diff_path = self.diff_path(image);
        write_opaque_png(&diff_path, &diff_image(&reference, actual))
            .map_err(|error| failure("write the diff", &diff_path, error))?;

        match mode {
            ReferenceMode::Check => Ok(ImageStatus::Differs(comparison)),
            ReferenceMode::Update => {
                write_opaque_png(&reference_path, actual)
                    .map_err(|error| failure("write the reference", &reference_path, error))?;
                Ok(ImageStatus::Replaced(comparison))
            }
        }
    }

    fn missing_reference(
        &self,
        image: &str,
        actual: &RgbaImage,
        mode: ReferenceMode,
    ) -> Result<ImageStatus, String> {
        match mode {
            ReferenceMode::Check => Ok(ImageStatus::Missing),
            ReferenceMode::Update => {
                let reference_path = self.reference_path(image);
                write_opaque_png(&reference_path, actual)
                    .map_err(|error| failure("write the reference", &reference_path, error))?;
                Ok(ImageStatus::Created)
            }
        }
    }
}

/// The outcome of checking one image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageStatus {
    Matches,
    Differs(PixelComparison),
    Missing,
    /// Update mode wrote a reference that did not exist.
    Created,
    /// Update mode replaced a reference whose pixels changed.
    Replaced(PixelComparison),
    Error(String),
}

impl ImageStatus {
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            ImageStatus::Differs(_) | ImageStatus::Missing | ImageStatus::Error(_)
        )
    }
}

/// Image names are lowercase kebab-case, so a name can never reach outside the two folders.
fn validate_image_name(image: &str) -> io::Result<()> {
    let is_kebab_case = !image.is_empty()
        && image
            .chars()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-');
    if is_kebab_case {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("the image name \"{image}\" is not lowercase kebab-case (a-z, 0-9 and -)"),
    ))
}

fn unusable_reference(path: &Path, error: PngFileError) -> String {
    format!(
        "Could not use the reference {}: {error}; {UNUSABLE_REFERENCE_REMEDY}",
        path.display()
    )
}

/// The file stems of the PNGs in `folder`; empty when the folder does not exist.
fn png_names(folder: &Path) -> io::Result<BTreeSet<String>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => return Err(error),
    };

    let mut names = BTreeSet::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension() != Some(OsStr::new(PNG_EXTENSION)) {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| invalid_name(&path))?;
        names.insert(name.to_owned());
    }
    Ok(names)
}

fn png_path(folder: &Path, image: &str) -> PathBuf {
    folder.join(format!("{image}.{PNG_EXTENSION}"))
}

fn failure(action: &str, path: &Path, error: impl Display) -> String {
    format!("Could not {action} {}: {error}", path.display())
}

fn invalid_name(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the file name {} is not valid Unicode", path.display()),
    )
}

/// `fs::remove_file` whose error names the file, keeping the error kind.
fn remove_file_with_path(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}
