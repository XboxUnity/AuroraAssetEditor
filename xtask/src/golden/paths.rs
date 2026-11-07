use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::harness::HARNESS_TARGET;

const VERBATIM_PATH_PREFIX: &str = r"\\?\";
const UNC_PATH_MARKER: &str = "UNC";

pub(crate) struct Paths {
    pub(super) root: PathBuf,
}

impl Paths {
    pub(crate) fn new() -> Result<Self> {
        let xtask_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = xtask_directory
            .parent()
            .context("xtask has no parent directory")?;
        Ok(Self {
            root: absolute(root)?,
        })
    }

    pub(crate) fn corpus(&self) -> PathBuf {
        self.root.join("fixtures").join("codec")
    }

    pub(crate) fn inputs(&self) -> PathBuf {
        self.corpus().join("inputs")
    }

    pub(crate) fn dll(&self) -> PathBuf {
        self.root
            .join("tools")
            .join("golden")
            .join("dll")
            .join("AuroraAsset.dll")
    }

    pub(super) fn harness(&self) -> PathBuf {
        self.root
            .join("target")
            .join(HARNESS_TARGET)
            .join("release")
            .join("golden-harness.exe")
    }
}

/// Canonical path without the `\\?\` prefix, which the DLL loader's altered search path mishandles.
pub(crate) fn absolute(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    let text = canonical.to_string_lossy();
    match text.strip_prefix(VERBATIM_PATH_PREFIX) {
        Some(plain) if !plain.starts_with(UNC_PATH_MARKER) => Ok(PathBuf::from(plain)),
        _ => Ok(canonical),
    }
}
