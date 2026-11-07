const ASSET_MAGIC: &[u8; 4] = b"RXEA";
const FSD_MAGIC: &[u8; 4] = b"FSDA";

/// What a file is, decided by its first four bytes and never by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Asset,
    Fsd,
    Unrecognised,
}

/// Fewer than four bytes is `Unrecognised`.
pub fn detect(first_bytes: &[u8]) -> FileKind {
    if first_bytes.starts_with(ASSET_MAGIC) {
        FileKind::Asset
    } else if first_bytes.starts_with(FSD_MAGIC) {
        FileKind::Fsd
    } else {
        FileKind::Unrecognised
    }
}
