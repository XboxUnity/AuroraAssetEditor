use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::asset_names::{EXE_EXTENSION, asset_name};
use super::release_platform::ReleasePlatform;

/// The Windows package is the release exe itself under its asset name: portable and unsigned.
pub fn package_windows(binary: &Path, version: &str, out: &Path) -> Result<PathBuf> {
    let target = out.join(asset_name(
        ReleasePlatform::WindowsX86_64,
        version,
        EXE_EXTENSION,
    ));
    fs::copy(binary, &target).with_context(|| {
        format!(
            "could not copy {} to {}",
            binary.display(),
            target.display()
        )
    })?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_package_copies_the_exe() {
        let work = tempfile::tempdir().unwrap();
        let binary = work.path().join("aurora-asset-editor.exe");
        fs::write(&binary, b"MZ fake exe").unwrap();
        let out = work.path().join("dist");
        fs::create_dir(&out).unwrap();

        let written = package_windows(&binary, "2.0.7", &out).unwrap();

        assert_eq!(
            written,
            out.join("AuroraAssetEditor-2.0.7-windows-x86_64.exe")
        );
        assert_eq!(fs::read(&written).unwrap(), b"MZ fake exe");
    }
}
