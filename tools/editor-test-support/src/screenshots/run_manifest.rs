use std::fs;
use std::io;
use std::path::Path;

use super::manifest_error::ManifestError;

pub const MANIFEST_FILE: &str = "manifest.txt";
pub const MANIFEST_FORMAT: u32 = 1;

const SEPARATOR: &str = ": ";

/// Describes one finished run: the platform it rendered on, who it is, when, and the screen filter
/// it ran with. Stored as `manifest.txt`, one `key: value` per line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunManifest {
    pub platform: String,
    pub label: String,
    pub rendered: String,
    pub filter: Option<String>,
}

impl RunManifest {
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "format: {MANIFEST_FORMAT}\nplatform: {}\nlabel: {}\nrendered: {}\n",
            self.platform, self.label, self.rendered
        );
        if let Some(filter) = &self.filter {
            text.push_str(&format!("filter: {filter}\n"));
        }
        text
    }

    pub fn from_text(text: &str) -> Result<RunManifest, ManifestError> {
        let mut format = None;
        let mut platform = None;
        let mut label = None;
        let mut rendered = None;
        let mut filter = None;

        for (index, line) in text.lines().enumerate() {
            let bad_line = || ManifestError::BadLine {
                line: index + 1,
                text: line.to_owned(),
            };
            let (key, value) = line.split_once(SEPARATOR).ok_or_else(bad_line)?;
            let slot = match key {
                "format" => &mut format,
                "platform" => &mut platform,
                "label" => &mut label,
                "rendered" => &mut rendered,
                "filter" => &mut filter,
                _ => return Err(bad_line()),
            };
            if slot.replace(value.to_owned()).is_some() {
                return Err(bad_line());
            }
            if key == "format" && value != MANIFEST_FORMAT.to_string() {
                return Err(ManifestError::UnknownFormat(value.to_owned()));
            }
        }

        format.ok_or(ManifestError::MissingField("format"))?;
        Ok(RunManifest {
            platform: platform.ok_or(ManifestError::MissingField("platform"))?,
            label: label.ok_or(ManifestError::MissingField("label"))?,
            rendered: rendered.ok_or(ManifestError::MissingField("rendered"))?,
            filter,
        })
    }

    pub fn write(&self, folder: &Path) -> io::Result<()> {
        let path = folder.join(MANIFEST_FILE);
        fs::create_dir_all(folder)?;
        fs::write(&path, self.to_text())
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
    }

    pub fn read(folder: &Path) -> Result<RunManifest, ManifestError> {
        let path = folder.join(MANIFEST_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => RunManifest::from_text(&text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(ManifestError::Missing(path))
            }
            Err(error) => Err(ManifestError::Unreadable { path, error }),
        }
    }
}
