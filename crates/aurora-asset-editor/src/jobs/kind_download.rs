use aurora_asset::AssetKind;
use aurora_console::ConsoleError;

use crate::editor::DecodedKind;

/// What the console had for one kind. `Ok(None)` means it has no such asset: no file, or a file without
/// entries of that kind (an asset that was removed is the empty asset).
#[derive(Debug)]
pub struct KindDownload {
    pub kind: AssetKind,
    pub result: Result<Option<Box<DecodedKind>>, ConsoleError>,
}
