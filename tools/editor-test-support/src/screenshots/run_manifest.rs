use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
        fs::create_dir_all(folder).map_err(|error| with_path(folder, error))?;
        fs::write(&path, self.to_text()).map_err(|error| with_path(&path, error))
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

fn with_path(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

#[derive(Debug)]
pub enum ManifestError {
    Missing(PathBuf),
    Unreadable { path: PathBuf, error: io::Error },
    UnknownFormat(String),
    BadLine { line: usize, text: String },
    MissingField(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => {
                write!(formatter, "{MANIFEST_FILE} is missing: {}", path.display())
            }
            Self::Unreadable { path, error } => write!(
                formatter,
                "{MANIFEST_FILE} could not be read ({}): {error}",
                path.display()
            ),
            Self::UnknownFormat(found) => write!(
                formatter,
                "{MANIFEST_FILE} has format {found}, this tool reads format {MANIFEST_FORMAT}"
            ),
            Self::BadLine { line, text } => write!(
                formatter,
                "{MANIFEST_FILE} line {line} is not a \"key: value\" line with a known key used once: {text}"
            ),
            Self::MissingField(field) => {
                write!(formatter, "{MANIFEST_FILE} has no \"{field}\" line")
            }
        }
    }
}

impl Error for ManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { error, .. } => Some(error),
            _ => None,
        }
    }
}
