/// One cover a service offers: the text shown for it and where to fetch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverResult {
    pub label: String,
    pub image_url: String,
    pub thumbnail_url: Option<String>,
}
