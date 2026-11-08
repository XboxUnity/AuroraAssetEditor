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

/// Lowercase letters and digits in words joined by single hyphens.
pub fn is_kebab_case(name: &str) -> bool {
    name.split('-').all(|word| {
        !word.is_empty()
            && word
                .chars()
                .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit())
    })
}

/// Image names are lowercase kebab-case, so a name can never reach outside the folders.
pub(super) fn validate_image_name(image: &str) -> io::Result<()> {
    if is_kebab_case(image) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "the image name \"{image}\" is not lowercase kebab-case (words of a-z and 0-9 joined by single -)"
        ),
    ))
}

fn invalid_name(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the file name {} is not valid Unicode", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_case_is_words_joined_by_single_hyphens() {
        for name in ["a", "shell-main", "icon-banner-2", "a1-b2-c3"] {
            assert!(is_kebab_case(name), "{name}");
            assert!(validate_image_name(name).is_ok(), "{name}");
        }
        for name in [
            "", "-", "a--b", "-a", "a-", "A", "a_b", "a b", "a/b", "..", "a.b",
        ] {
            assert!(!is_kebab_case(name), "{name}");
            assert!(validate_image_name(name).is_err(), "{name}");
        }
    }
}
