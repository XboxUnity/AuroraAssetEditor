use super::{FakeFeatures, ListingStyle};

const DEFAULT_CREDENTIAL: &str = "xboxftp";
const DEFAULT_SITE_REVISION_REPLY: &str = "200 1655";

/// What Aurora's failed-login reply carries in place of its real MD5 values.
pub const FAKE_INPUT_HASH: &str = "11111111111111111111111111111111";
pub const FAKE_STORED_HASH: &str = "22222222222222222222222222222222";

/// The default is Aurora's FtpDll, whatever the fields not set here.
#[derive(Debug, Clone)]
pub struct FakeConsoleConfig {
    pub username: String,
    pub password: String,
    /// The full reply to `SITE REVISION`; `None` answers `500 Unknown command`.
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
            features: FakeFeatures::default(),
            listing_style: ListingStyle::default(),
        }
    }
}
