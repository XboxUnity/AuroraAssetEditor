use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const UI_SCREENSHOTS_ZIP: &str = "ui-screenshots.zip";
pub const USAGE_LINE: &str = "cargo xtask ui-zip <folder> <zip>";

// Mirrors `editor-test-support`, which xtask does not depend on.
pub(crate) const MANIFEST_FILE: &str = "manifest.txt";

/// Zips every platform subfolder of `folder` as `<platform>/<path>`, and returns the platforms.
/// A subfolder that is not a finished run (no `manifest.txt`) is an error.
pub fn write_zip(folder: &Path, zip_path: &Path) -> Result<Vec<String>> {
    let platforms = platform_folders(folder)?;
    let file = File::create(zip_path)
        .with_context(|| format!("could not create {}", zip_path.display()))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, path) in &platforms {
        add_folder(&mut writer, options, path, name)?;
    }
    writer
        .finish()
        .with_context(|| format!("could not finish {}", zip_path.display()))?;
    Ok(platforms.into_iter().map(|(name, _)| name).collect())
}

fn platform_folders(folder: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut platforms = Vec::new();
    for entry in sorted_entries(folder)? {
        if !entry.is_dir() {
            bail!(
                "{} is a file, but the folder holds only platform folders",
                entry.display()
            );
        }
        let name = file_name(&entry)?;
        if !entry.join(MANIFEST_FILE).is_file() {
            bail!(
                "{} holds no {MANIFEST_FILE}, so it is not a finished screenshot run",
                entry.display()
            );
        }
        platforms.push((name, entry));
    }
    if platforms.is_empty() {
        bail!("{} holds no platform folders", folder.display());
    }
    Ok(platforms)
}

fn sorted_entries(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    let listing =
        fs::read_dir(folder).with_context(|| format!("could not read {}", folder.display()))?;
    for entry in listing {
        let entry = entry.with_context(|| format!("could not read {}", folder.display()))?;
        entries.push(entry.path());
    }
    entries.sort();
    Ok(entries)
}

fn file_name(path: &Path) -> Result<String> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => Ok(name.to_owned()),
        None => bail!("{} has a name that is not valid UTF-8", path.display()),
    }
}

fn add_folder(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    folder: &Path,
    entry_prefix: &str,
) -> Result<()> {
    for path in sorted_entries(folder)? {
        let entry_name = format!("{entry_prefix}/{}", file_name(&path)?);
        if path.is_dir() {
            add_folder(writer, options, &path, &entry_name)?;
        } else {
            let bytes =
                fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
            writer
                .start_file(&entry_name, options)
                .with_context(|| format!("could not add {entry_name} to the zip"))?;
            writer
                .write_all(&bytes)
                .with_context(|| format!("could not write {entry_name} to the zip"))?;
        }
    }
    Ok(())
}

/// Unpacks `zip_path` into `destination`; an entry whose path could leave it is refused.
pub fn extract_zip(zip_path: &Path, destination: &Path) -> Result<()> {
    let file =
        File::open(zip_path).with_context(|| format!("could not open {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} is not a readable zip", zip_path.display()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("could not read entry {index} of {}", zip_path.display()))?;
        let Some(relative) = entry.enclosed_name() else {
            let name = match entry.name() {
                Ok(name) => name.into_owned(),
                Err(error) => format!("(unreadable name: {error})"),
            };
            bail!("the zip entry `{name}` leaves the destination folder");
        };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)
                .with_context(|| format!("could not create {}", target.display()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let mut output = File::create(&target)
            .with_context(|| format!("could not create {}", target.display()))?;
        io::copy(&mut entry, &mut output)
            .with_context(|| format!("could not extract {}", target.display()))?;
    }
    Ok(())
}

pub fn run(folder: &str, zip_path: &str) -> Result<()> {
    let platforms = write_zip(Path::new(folder), Path::new(zip_path))?;
    println!("Wrote {zip_path}: {}", platforms.join(", "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn zip_round_trip_keeps_platform_folders() {
        let source = tempfile::tempdir().unwrap();
        let files: [(&str, &[u8]); 4] = [
            ("windows-x86_64/manifest.txt", b"manifest"),
            ("windows-x86_64/index.html", b"<html>"),
            ("windows-x86_64/images/a.png", &[0, 1, 2, 255]),
            ("linux-x86_64/manifest.txt", b"other"),
        ];
        for (name, bytes) in files {
            write(source.path(), name, bytes);
        }
        let work = tempfile::tempdir().unwrap();
        let zip_path = work.path().join(UI_SCREENSHOTS_ZIP);

        let platforms = write_zip(source.path(), &zip_path).unwrap();
        let unpacked = work.path().join("unpacked");
        extract_zip(&zip_path, &unpacked).unwrap();

        assert_eq!(platforms, ["linux-x86_64", "windows-x86_64"]);
        for (name, bytes) in files {
            assert_eq!(fs::read(unpacked.join(name)).unwrap(), bytes, "{name}");
        }
    }

    #[test]
    fn folder_without_manifest_is_refused() {
        let source = tempfile::tempdir().unwrap();
        write(source.path(), "windows-x86_64/manifest.txt", b"m");
        write(source.path(), "macos-aarch64/index.html", b"<html>");
        let zip_path = source.path().join("out.zip");

        let message = write_zip(source.path(), &zip_path).unwrap_err().to_string();

        assert!(message.contains("macos-aarch64"), "{message}");
    }

    #[test]
    fn folder_without_platforms_is_refused() {
        let source = tempfile::tempdir().unwrap();
        let zip_path = source.path().join("out.zip");

        let message = write_zip(source.path(), &zip_path).unwrap_err().to_string();

        assert!(message.contains("no platform folders"), "{message}");
    }

    #[test]
    fn stray_file_beside_the_platform_folders_is_refused() {
        let source = tempfile::tempdir().unwrap();
        write(source.path(), "windows-x86_64/manifest.txt", b"m");
        write(source.path(), "stray.txt", b"x");
        let zip_path = source.path().join("out.zip");

        let message = write_zip(source.path(), &zip_path).unwrap_err().to_string();

        assert!(message.contains("stray.txt"), "{message}");
        assert!(message.contains("is a file"), "{message}");
    }

    #[test]
    fn entry_outside_the_destination_is_refused() {
        let work = tempfile::tempdir().unwrap();
        let zip_path = work.path().join("evil.zip");
        let mut writer = ZipWriter::new(File::create(&zip_path).unwrap());
        writer
            .start_file("../escape.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"escaped").unwrap();
        writer.finish().unwrap();
        let destination = work.path().join("inside");
        fs::create_dir(&destination).unwrap();

        let message = extract_zip(&zip_path, &destination)
            .unwrap_err()
            .to_string();

        assert!(message.contains("../escape.txt"), "{message}");
        assert!(!work.path().join("escape.txt").exists());
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    }
}
