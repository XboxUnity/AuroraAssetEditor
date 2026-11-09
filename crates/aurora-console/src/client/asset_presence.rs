use aurora_asset::AssetKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetPresence {
    pub cover: bool,
    pub background: bool,
    pub icon_banner: bool,
    pub screenshots: bool,
}

impl AssetPresence {
    pub const NONE: AssetPresence = AssetPresence {
        cover: false,
        background: false,
        icon_banner: false,
        screenshots: false,
    };

    pub fn has(&self, kind: AssetKind) -> bool {
        match kind {
            AssetKind::Cover => self.cover,
            AssetKind::Background => self.background,
            AssetKind::IconBanner => self.icon_banner,
            AssetKind::Screenshots => self.screenshots,
        }
    }
}
