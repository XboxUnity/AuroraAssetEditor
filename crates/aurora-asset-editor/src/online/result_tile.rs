use aurora_asset::RgbaImage;
use aurora_online::CoverResult;

/// One image of a result: not loaded yet (or not wanted yet), loaded and decoded, or failed.
#[derive(Debug, Clone)]
pub enum TileImage {
    Pending,
    Loaded {
        pixels: RgbaImage,
        /// The downloaded file as it came, kept so a cover can be made from it.
        bytes: Vec<u8>,
    },
    Failed,
}

/// A result in the grid: the service's answer and its two images. The full image is only fetched once the
/// tile is selected.
#[derive(Debug, Clone)]
pub struct ResultTile {
    pub result: CoverResult,
    pub thumbnail: TileImage,
    pub full: TileImage,
}

impl ResultTile {
    pub fn new(result: CoverResult) -> ResultTile {
        ResultTile {
            result,
            thumbnail: TileImage::Pending,
            full: TileImage::Pending,
        }
    }

    /// The thumbnail is loaded from `thumbnail_url`, else from the full image's address.
    pub fn thumbnail_source_url(&self) -> &str {
        self.result
            .thumbnail_url
            .as_deref()
            .unwrap_or(&self.result.image_url)
    }
}
