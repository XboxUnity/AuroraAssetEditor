use super::{FakeFeatures, ListingStyle};

const DEFAULT_CREDENTIAL: &str = "xboxftp";
const DEFAULT_SITE_REVISION_REPLY: &str = "200 1234";

#[derive(Debug, Clone)]
pub struct FakeConsoleConfig {
    pub username: String,
    pub password: String,
    /// The full reply to `SITE`; `None` answers `500 Unknown command`.
    pub site_revision_reply: Option<String>,
    pub features: FakeFeatures,
    pub listing_style: ListingStyle,
}

impl Default for FakeConsoleConfig {
    fn default() -> Self {
        FakeConsoleConfig {
            username: DEFAULT_CREDENTIAL.to_owned(),
            password: DEFAULT_CREDENTIAL.to_owned(),
            site_revision_reply: Some(DEFAULT_SITE_REVISION_REPLY.to_owned()),
            features: FakeFeatures::Basic,
            listing_style: ListingStyle::Unix,
        }
    }
}
