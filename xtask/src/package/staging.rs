//! Laying out a package's files in its work folder.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// The mode of the app's binary in the Linux packages and the macOS bundle: `rwxr-xr-x`.
#[cfg(unix)]
const EXECUTABLE_MODE: u32 = 0o755;

/// Empties `folder`, so that nothing from an earlier run ends up in a package.
pub fn recreate_folder(folder: &Path) -> Result<()> {
    if folder.exists() {
        fs::remove_dir_all(folder)
            .with_context(|| format!("could not empty {}", folder.display()))?;
    }
    create_folder(folder)
}

pub fn create_folder(folder: &Path) -> Result<()> {
    fs::create_dir_all(folder).with_context(|| format!("could not create {}", folder.display()))
}

pub fn copy_file(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to)
        .with_context(|| format!("could not copy {} to {}", from.display(), to.display()))?;
    Ok(())
}

/// Copies `from` into a new folder `to`, file modes included.
pub fn copy_folder(from: &Path, to: &Path) -> Result<()> {
    create_folder(to)?;
    let listing =
        fs::read_dir(from).with_context(|| format!("could not read {}", from.display()))?;
    for entry in listing {
        let entry = entry.with_context(|| format!("could not read {}", from.display()))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_folder(&source, &target)?;
        } else {
            copy_file(&source, &target)?;
        }
    }
    Ok(())
}

pub fn write_text(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text).with_context(|| format!("could not write {}", path.display()))
}

#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(EXECUTABLE_MODE))
        .with_context(|| format!("could not make {} executable", path.display()))
}

#[cfg(not(unix))]
pub fn make_executable(path: &Path) -> Result<()> {
    anyhow::bail!(
        "could not make {} executable: file modes exist only on Unix, so build this package on Linux or macOS",
        path.display()
    )
}

/// Creates the symbolic link `link` pointing at `target`, which is relative to the link's folder.
#[cfg(unix)]
pub fn symlink(target: &str, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("could not link {} to {target}", link.display()))
}

#[cfg(not(unix))]
pub fn symlink(target: &str, link: &Path) -> Result<()> {
    anyhow::bail!(
        "could not link {} to {target}: build this package on Linux or macOS",
        link.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recreated_folder_is_empty_and_copies_keep_the_layout() {
        let work = tempfile::tempdir().unwrap();
        let source = work.path().join("Bundle.app");
        create_folder(&source.join("Contents/MacOS")).unwrap();
        write_text(&source.join("Contents/Info.plist"), "plist").unwrap();
        write_text(&source.join("Contents/MacOS/app"), "binary").unwrap();
        let staging = work.path().join("dmg");
        create_folder(&staging).unwrap();
        write_text(&staging.join("stale.txt"), "old").unwrap();

        recreate_folder(&staging).unwrap();
        copy_folder(&source, &staging.join("Bundle.app")).unwrap();

        assert!(!staging.join("stale.txt").exists());
        let copy = staging.join("Bundle.app/Contents");
        assert_eq!(
            fs::read_to_string(copy.join("Info.plist")).unwrap(),
            "plist"
        );
        assert_eq!(
            fs::read_to_string(copy.join("MacOS/app")).unwrap(),
            "binary"
        );
    }
}
