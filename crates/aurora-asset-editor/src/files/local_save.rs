use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aurora_asset::AssetKind;
use aurora_console::{HexId, asset_file_name};

use crate::editor::{AssetDocument, EditorPages};

/// Save All writes in the order Aurora's files are listed everywhere else.
const SAVE_ALL_ORDER: [AssetKind; 4] = [
    AssetKind::Cover,
    AssetKind::Background,
    AssetKind::IconBanner,
    AssetKind::Screenshots,
];

/// A file could not be written; `written` lists the files Save All had written before it.
#[derive(Debug)]
pub struct LocalSaveError {
    pub path: PathBuf,
    pub written: Vec<PathBuf>,
    pub source: io::Error,
}

impl fmt::Display for LocalSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not write {}", self.path.display())
    }
}

impl Error for LocalSaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Writes the page's stored bytes. Screenshots are packed into positions 1, 2, 3, ... first, by moving
/// stored bytes, so the file and the page agree (the old app only compacted the file). Once the file is
/// written, the page has no unsaved changes.
pub fn save_page(document: &mut AssetDocument, path: &Path) -> Result<(), LocalSaveError> {
    write_page(document, path, Vec::new())
}

/// A page is written when it has content or its file does not exist yet, so an
/// empty page never overwrites an existing file. Stops at the first failure.
pub fn save_all_to_folder(
    pages: &mut EditorPages,
    folder: &Path,
    title_id: &HexId,
) -> Result<Vec<PathBuf>, LocalSaveError> {
    let mut written = Vec::new();
    for kind in SAVE_ALL_ORDER {
        let path = folder.join(asset_file_name(kind, title_id));
        let document = pages.get_mut(kind);
        let exists = path.try_exists().map_err(|source| LocalSaveError {
            path: path.clone(),
            written: written.clone(),
            source,
        })?;
        if document.has_content() || !exists {
            write_page(document, &path, written.clone())?;
            written.push(path);
        }
    }
    Ok(written)
}

fn write_page(
    document: &mut AssetDocument,
    path: &Path,
    written: Vec<PathBuf>,
) -> Result<(), LocalSaveError> {
    if document.kind() == AssetKind::Screenshots {
        document.compact_screenshots();
    }
    let change_count = document.change_count();
    fs::write(path, document.to_bytes()).map_err(|source| LocalSaveError {
        path: path.to_path_buf(),
        written,
        source,
    })?;
    document.mark_saved(change_count);
    Ok(())
}
