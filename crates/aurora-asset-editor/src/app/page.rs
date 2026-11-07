use aurora_asset::AssetKind;

/// The sidebar's pages: the four editor pages, then the two sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Cover,
    Background,
    IconBanner,
    Screenshots,
    Online,
    Console,
}

impl Page {
    /// The editor page that shows `kind`.
    pub fn of_kind(kind: AssetKind) -> Page {
        match kind {
            AssetKind::Cover => Page::Cover,
            AssetKind::Background => Page::Background,
            AssetKind::IconBanner => Page::IconBanner,
            AssetKind::Screenshots => Page::Screenshots,
        }
    }
}
