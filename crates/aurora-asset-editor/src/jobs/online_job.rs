use std::error::Error;
use std::fmt;

use aurora_asset::RgbaImage;
use aurora_image::{ImportError, decode_image};
use aurora_online::{CoverQuery, CoverResult, CoverSource, OnlineClient, OnlineError};

use crate::online::SearchGate;

/// A request to XboxUnity or the Internet Archive.
#[derive(Debug)]
pub enum OnlineJob {
    Search {
        generation: u64,
        source: CoverSource,
        query: CoverQuery,
    },
    /// Fetches the image shown on a result's tile, unless `gate` says a newer search replaced its own.
    Thumbnail {
        generation: u64,
        index: usize,
        url: String,
        gate: SearchGate,
    },
    /// Fetches the image of the selected result, unless `gate` says a newer search replaced its own.
    Full {
        generation: u64,
        index: usize,
        url: String,
        gate: SearchGate,
    },
}

#[derive(Debug)]
pub enum OnlineOutcome {
    Found {
        generation: u64,
        query_text: String,
        result: Result<Vec<CoverResult>, OnlineError>,
    },
    Thumbnail {
        generation: u64,
        index: usize,
        result: Result<FetchedImage, FetchError>,
    },
    Full {
        generation: u64,
        index: usize,
        result: Result<FetchedImage, FetchError>,
    },
    /// A thumbnail or full image of a replaced search; nothing was fetched.
    DownloadSkipped { generation: u64 },
}

/// A downloaded image: decoded for display, with the file it came from.
#[derive(Debug)]
pub struct FetchedImage {
    pub pixels: RgbaImage,
    pub bytes: Vec<u8>,
}

/// Why an image could not be fetched. It reads as the error it wraps.
#[derive(Debug)]
pub enum FetchError {
    Online(OnlineError),
    Image(ImportError),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Online(error) => error.fmt(f),
            FetchError::Image(error) => error.fmt(f),
        }
    }
}

impl Error for FetchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FetchError::Online(error) => error.source(),
            FetchError::Image(error) => error.source(),
        }
    }
}

impl OnlineJob {
    pub fn run(self, client: &OnlineClient) -> OnlineOutcome {
        match self {
            OnlineJob::Search {
                generation,
                source,
                query,
            } => OnlineOutcome::Found {
                generation,
                query_text: query_text(&query),
                result: client.find_covers(source, &query),
            },
            OnlineJob::Thumbnail {
                generation,
                index,
                url,
                gate,
            } => {
                if gate.is_superseded(generation) {
                    return OnlineOutcome::DownloadSkipped { generation };
                }
                OnlineOutcome::Thumbnail {
                    generation,
                    index,
                    result: fetch_image(client, &url),
                }
            }
            OnlineJob::Full {
                generation,
                index,
                url,
                gate,
            } => {
                if gate.is_superseded(generation) {
                    return OnlineOutcome::DownloadSkipped { generation };
                }
                OnlineOutcome::Full {
                    generation,
                    index,
                    result: fetch_image(client, &url),
                }
            }
        }
    }
}

pub(crate) fn query_text(query: &CoverQuery) -> String {
    match query {
        CoverQuery::TitleId(title_id) => title_id.to_string(),
        CoverQuery::Keywords(text) => text.clone(),
    }
}

fn fetch_image(client: &OnlineClient, url: &str) -> Result<FetchedImage, FetchError> {
    let bytes = client.download_image(url).map_err(FetchError::Online)?;
    let pixels = decode_image(&bytes, None).map_err(FetchError::Image)?;
    Ok(FetchedImage { pixels, bytes })
}
