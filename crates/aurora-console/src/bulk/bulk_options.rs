use aurora_asset::Compression;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BulkOptions {
    /// Skips the check for an existing cover and overwrites it.
    pub replace_existing: bool,
    pub compression: Compression,
}
