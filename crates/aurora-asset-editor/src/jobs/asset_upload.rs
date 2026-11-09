use aurora_asset::AssetKind;

/// One asset file to put on the console: the bytes the page's `AssetFile` writes, taken at the page's
/// `change_count`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetUpload {
    pub kind: AssetKind,
    pub bytes: Vec<u8>,
    pub change_count: u64,
}
