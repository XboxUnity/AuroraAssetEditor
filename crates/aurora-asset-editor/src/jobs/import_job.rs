use std::fs;
use std::path::PathBuf;

use aurora_asset::{AssetSlot, Compression, ScreenshotNumber};
use aurora_image::{ImageTarget, decode_image};

use crate::editor::PreparedImage;
use crate::files::file_display_name;

use super::ImportFailure;

/// Reads, decodes, resizes and encodes images for page entries, each for the slot it was queued for.
#[derive(Debug)]
pub struct ImportJob {
    pub requests: Vec<ImportRequest>,
    /// "Auto Resize Images" as it is when the job is queued; it applies to the requests that allow a resize.
    pub auto_resize: bool,
    /// "Use Compression" as it is when the job is queued.
    pub compression: Compression,
}

#[derive(Debug)]
pub struct ImportRequest {
    pub source: ImageSource,
    pub slot: ImportSlot,
    /// Whether Auto Resize may stretch this image: files yes, results fetched from elsewhere no (spec 6.3).
    pub resize: bool,
}

/// Where an imported image goes. A free screenshot position is picked when the result is put on the page,
/// so imports queued one after another never pick the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSlot {
    Slot(AssetSlot),
    /// `preferred` when it is still free then, else the first free position.
    FreeScreenshot {
        preferred: Option<ScreenshotNumber>,
    },
}

impl ImportSlot {
    fn target(self) -> Option<ImageTarget> {
        match self {
            ImportSlot::Slot(slot) => ImageTarget::for_slot(slot),
            ImportSlot::FreeScreenshot { .. } => Some(ImageTarget::Screenshot),
        }
    }
}

#[derive(Debug)]
pub enum ImageSource {
    File(PathBuf),
    Bytes { label: String, bytes: Vec<u8> },
}

/// What became of one request; `label` names the image in messages.
#[derive(Debug)]
pub struct ImportResult {
    pub slot: ImportSlot,
    pub label: String,
    pub result: Result<PreparedImage, ImportFailure>,
}

impl ImportJob {
    pub fn run(self) -> Vec<ImportResult> {
        let ImportJob {
            requests,
            auto_resize,
            compression,
        } = self;
        requests
            .into_iter()
            .map(|request| import(request, auto_resize, compression))
            .collect()
    }
}

fn import(request: ImportRequest, auto_resize: bool, compression: Compression) -> ImportResult {
    let ImportRequest {
        source,
        slot,
        resize,
    } = request;
    let label = match &source {
        ImageSource::File(path) => file_display_name(path),
        ImageSource::Bytes { label, .. } => label.clone(),
    };
    let result = prepare(&source, slot, auto_resize && resize, compression);
    ImportResult {
        slot,
        label,
        result,
    }
}

fn prepare(
    source: &ImageSource,
    slot: ImportSlot,
    resize: bool,
    compression: Compression,
) -> Result<PreparedImage, ImportFailure> {
    let target = slot.target().ok_or(ImportFailure::NoPicture)?;
    let pixels = match source {
        ImageSource::File(path) => {
            let bytes = fs::read(path).map_err(ImportFailure::Read)?;
            let name = path.file_name().and_then(|name| name.to_str());
            decode_image(&bytes, name)
        }
        ImageSource::Bytes { bytes, .. } => decode_image(bytes, None),
    }
    .map_err(ImportFailure::Decode)?;
    PreparedImage::prepare(pixels, target, resize, compression).map_err(ImportFailure::Encode)
}
