use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(super) const PNG_EXTENSION: &str = "png";

pub(super) fn png_path(folder: &Path, image: &str) -> PathBuf {
    folder.join(format!("{image}.{PNG_EXTENSION}"))
}

/// The file stems of the PNGs in `folder`; empty when the folder does not exist.
pub(super) fn png_names(folder: &Path) -> io::Result<BTreeSet<String>> {
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

/// Image names are lowercase kebab-case, so a name can never reach outside the folders.
pub(super) fn validate_image_name(image: &str) -> io::Result<()> {
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

fn invalid_name(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the file name {} is not valid Unicode", path.display()),
    )
}
