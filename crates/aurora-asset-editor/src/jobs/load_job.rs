use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aurora_asset::fsd::{FsdFile, FsdImage};
use aurora_asset::{AssetFile, AssetSlot, Compression};
use aurora_image::decode_image;

use crate::editor::{DecodedKind, PreparedImage, decode_kind};
use crate::files::{
    FileKind, FileProblem, FsdImages, ImageLoadError, LoadProblem, LoadedFile, LoadedFiles, detect,
    screenshot_label, slot_label,
};

/// Reads and decodes asset files, whatever their names say: the content decides if one is an `.asset` or an
/// FSD file.
#[derive(Debug)]
pub struct LoadJob {
    pub paths: Vec<PathBuf>,
    /// "Use Compression" as it is when the job is queued; it applies to the images of FSD files.
    pub compression: Compression,
}

impl LoadJob {
    pub fn run(self) -> LoadedFiles {
        let mut loaded = LoadedFiles::default();
        for path in self.paths {
            match load_file(&path, self.compression) {
                Ok(file) => loaded.files.push(file),
                Err(problem) => loaded.problems.push(FileProblem { path, problem }),
            }
        }
        loaded
    }
}

fn load_file(path: &Path, compression: Compression) -> Result<LoadedFile, LoadProblem> {
    let bytes = read_file(path)?;
    let path = path.to_path_buf();
    match detect(&bytes) {
        FileKind::Asset => {
            let decoded = Box::new(load_asset(&bytes)?);
            Ok(LoadedFile::Asset { path, decoded })
        }
        FileKind::Fsd => {
            let images = Box::new(load_fsd(&bytes, compression)?);
            Ok(LoadedFile::Fsd { path, images })
        }
        FileKind::Unrecognised => Err(LoadProblem::Unrecognised),
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, LoadProblem> {
    if path.is_dir() {
        return Err(LoadProblem::Folder);
    }
    fs::read(path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => LoadProblem::NotFound,
        _ => LoadProblem::Read(error),
    })
}

/// Only the page that comes first by `primary_kind` is taken, as in the old app.
fn load_asset(bytes: &[u8]) -> Result<DecodedKind, LoadProblem> {
    let file = AssetFile::parse(bytes).map_err(LoadProblem::Asset)?;
    let kind = file.primary_kind().ok_or(LoadProblem::NoAssets)?;
    Ok(decode_kind(&file, kind))
}

fn load_fsd(bytes: &[u8], compression: Compression) -> Result<FsdImages, LoadProblem> {
    let fsd = FsdFile::parse(bytes).map_err(LoadProblem::Fsd)?;
    let nothing_present = fsd.cover.is_none()
        && fsd.background.is_none()
        && fsd.icon.is_none()
        && fsd.banner.is_none()
        && fsd.screenshots.is_empty();
    if nothing_present {
        return Err(LoadProblem::NoAssets);
    }

    let mut unreadable = Vec::new();
    let mut prepare = |name: String, image: &FsdImage| match prepare_image(image, compression) {
        Ok(prepared) => Some(prepared),
        Err(error) => {
            unreadable.push((name, error));
            None
        }
    };
    let cover = fsd
        .cover
        .as_ref()
        .and_then(|image| prepare(slot_label(AssetSlot::Cover), image));
    let background = fsd
        .background
        .as_ref()
        .and_then(|image| prepare(slot_label(AssetSlot::Background), image));
    let icon = fsd
        .icon
        .as_ref()
        .and_then(|image| prepare(slot_label(AssetSlot::Icon), image));
    let banner = fsd
        .banner
        .as_ref()
        .and_then(|image| prepare(slot_label(AssetSlot::Banner), image));
    let screenshots = fsd
        .screenshots
        .iter()
        .enumerate()
        .filter_map(|(index, image)| prepare(screenshot_label(index + 1), image))
        .collect();
    Ok(FsdImages {
        cover,
        background,
        icon,
        banner,
        screenshots,
        unreadable,
    })
}

fn prepare_image(
    image: &FsdImage,
    compression: Compression,
) -> Result<PreparedImage, ImageLoadError> {
    let (FsdImage::Dds(bytes) | FsdImage::Encoded(bytes)) = image;
    let pixels = decode_image(bytes, None).map_err(ImageLoadError::Import)?;
    PreparedImage::without_resize(pixels, compression).map_err(ImageLoadError::Encode)
}
