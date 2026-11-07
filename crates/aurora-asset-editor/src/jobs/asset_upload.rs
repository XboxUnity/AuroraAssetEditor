use aurora_asset::AssetKind;

/// One asset file to put on the console: the bytes the page's `AssetFile` writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetUpload {
    pub kind: AssetKind,
    pub bytes: Vec<u8>,
}
