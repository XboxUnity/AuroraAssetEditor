use aurora_asset::RgbaImage;

use crate::ids::HexId;

/// Where bulk covers come from; `Ok(None)` means the title has no cover there.
pub trait CoverProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn random_cover(&mut self, title_id: &HexId) -> Result<Option<RgbaImage>, Self::Error>;
}
