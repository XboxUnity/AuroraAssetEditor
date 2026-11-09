use std::error::Error;
use std::fmt;

use aurora_asset::RgbaImage;
use aurora_console::{CoverProvider, HexId};
use aurora_image::{ImportError, decode_image, encoder_pixels};
use aurora_online::{CoverSource, OnlineClient, OnlineError, QueryError, TitleId};

/// Bulk covers from an online source: a random one of the title's covers, as the pixels `encode` takes.
/// Covers are never resized, so they stay as the source made them.
pub struct OnlineCoverProvider {
    client: OnlineClient,
    source: CoverSource,
    rng: fastrand::Rng,
}

impl OnlineCoverProvider {
    pub fn new(client: OnlineClient, source: CoverSource, rng: fastrand::Rng) -> Self {
        OnlineCoverProvider {
            client,
            source,
            rng,
        }
    }
}

impl CoverProvider for OnlineCoverProvider {
    type Error = CoverProviderError;

    fn random_cover(&mut self, title_id: &HexId) -> Result<Option<RgbaImage>, CoverProviderError> {
        let title_id = TitleId::parse(title_id.as_str()).map_err(CoverProviderError::BadTitleId)?;
        let bytes = self
            .client
            .random_cover(self.source, title_id, &mut self.rng)
            .map_err(CoverProviderError::Online)?;
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        let image = decode_image(&bytes, None).map_err(CoverProviderError::Image)?;
        Ok(Some(encoder_pixels(&image)))
    }
}

#[derive(Debug)]
pub enum CoverProviderError {
    Online(OnlineError),
    BadTitleId(QueryError),
    Image(ImportError),
}

impl fmt::Display for CoverProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Online(_) => write!(f, "the online source could not provide a cover"),
            Self::BadTitleId(_) => write!(f, "the TitleID cannot be searched for online"),
            Self::Image(_) => write!(f, "the downloaded cover could not be read as an image"),
        }
    }
}

impl Error for CoverProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Online(source) => Some(source),
            Self::BadTitleId(source) => Some(source),
            Self::Image(source) => Some(source),
        }
    }
}
