use super::fetch_format::FetchFormat;

/// The app's "Use Compression" toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Dxt,
    None,
}

impl Compression {
    pub(super) fn fetch_format(self) -> FetchFormat {
        match self {
            Self::Dxt => FetchFormat::LINEAR_DXT4,
            Self::None => FetchFormat::LINEAR_ARGB8888,
        }
    }
}
