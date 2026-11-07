use std::time::Duration;

use ureq::Agent;

use crate::OnlineError;
use crate::archive::{archive_cover, archive_listing_url, parse_archive_variants};
use crate::client_options::ClientOptions;
use crate::cover_result::CoverResult;
use crate::endpoints::Endpoints;
use crate::query::{CoverQuery, CoverSource, TitleId};
use crate::xboxunity::{parse_xboxunity_covers, xboxunity_search_url};

/// The old `WebClient` default.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(100);

const NOT_FOUND: u16 = 404;

/// Finds and downloads covers. Every request is attempted once; failures are returned.
pub struct OnlineClient {
    endpoints: Endpoints,
    agent: Agent,
    max_response_bytes: u64,
}

impl OnlineClient {
    /// Uses the environment and Windows system proxy settings, like the old `WebClient`.
    pub fn new(endpoints: Endpoints, timeout: Duration) -> Self {
        OnlineClient::with_options(
            endpoints,
            ClientOptions {
                timeout,
                ..ClientOptions::default()
            },
        )
    }

    /// Ignores every proxy setting, for talking to a test server on 127.0.0.1.
    pub fn without_proxy(endpoints: Endpoints, timeout: Duration) -> Self {
        OnlineClient::with_options(
            endpoints,
            ClientOptions {
                timeout,
                use_proxy: false,
                ..ClientOptions::default()
            },
        )
    }

    pub fn with_options(endpoints: Endpoints, options: ClientOptions) -> Self {
        let builder = Agent::config_builder().timeout_global(Some(options.timeout));
        let config = if options.use_proxy {
            builder.build()
        } else {
            builder.proxy(None).build()
        };
        OnlineClient {
            endpoints,
            agent: config.into(),
            max_response_bytes: options.max_response_bytes,
        }
    }

    pub fn find_covers(
        &self,
        source: CoverSource,
        query: &CoverQuery,
    ) -> Result<Vec<CoverResult>, OnlineError> {
        match source {
            CoverSource::XboxUnity => self.find_xboxunity_covers(query),
            CoverSource::InternetArchive => self.find_archive_covers(query),
        }
    }

    pub fn download_image(&self, url: &str) -> Result<Vec<u8>, OnlineError> {
        let image = self.get(url)?;
        if image.is_empty() {
            return Err(OnlineError::EmptyImage {
                url: url.to_string(),
            });
        }
        Ok(image)
    }

    /// One of the title's covers, chosen with `rng` like the old `Random.Next(0, count)`.
    pub fn random_cover(
        &self,
        source: CoverSource,
        title_id: TitleId,
        rng: &mut fastrand::Rng,
    ) -> Result<Option<Vec<u8>>, OnlineError> {
        let covers = self.find_covers(source, &CoverQuery::TitleId(title_id))?;
        if covers.is_empty() {
            return Ok(None);
        }
        let chosen = &covers[rng.usize(..covers.len())];
        self.download_image(&chosen.image_url).map(Some)
    }

    fn find_xboxunity_covers(&self, query: &CoverQuery) -> Result<Vec<CoverResult>, OnlineError> {
        let url = xboxunity_search_url(&self.endpoints.xboxunity_covers, query);
        let json = self.get(&url)?;
        parse_xboxunity_covers(&json).map_err(|error| OnlineError::InvalidResponse {
            url,
            reason: error.to_string(),
        })
    }

    fn find_archive_covers(&self, query: &CoverQuery) -> Result<Vec<CoverResult>, OnlineError> {
        let CoverQuery::TitleId(title_id) = query else {
            return Err(OnlineError::KeywordsNeedXboxUnity);
        };
        let base = &self.endpoints.archive_covers;
        let url = archive_listing_url(base, *title_id);
        let listing = match self.get(&url) {
            Ok(listing) => listing,
            // A title missing from the dump is a normal outcome, not an error.
            Err(OnlineError::HttpStatus {
                status: NOT_FOUND, ..
            }) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let html = String::from_utf8_lossy(&listing);
        Ok(parse_archive_variants(&html)
            .iter()
            .map(|variant| archive_cover(base, *title_id, variant))
            .collect())
    }

    fn get(&self, url: &str) -> Result<Vec<u8>, OnlineError> {
        let request_error = |source: ureq::Error| match source {
            ureq::Error::StatusCode(status) => OnlineError::HttpStatus {
                url: url.to_string(),
                status,
            },
            other => OnlineError::Request {
                url: url.to_string(),
                source: Box::new(other),
            },
        };
        let mut response = self.agent.get(url).call().map_err(request_error)?;
        response
            .body_mut()
            .with_config()
            .limit(self.max_response_bytes)
            .read_to_vec()
            .map_err(request_error)
    }
}
