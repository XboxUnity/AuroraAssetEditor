use aurora_asset::{AssetFile, AssetKind, AssetSlot, Compression, encode};

use super::{BulkEntry, BulkError, BulkFailure, BulkOutcome, BulkReport, CoverProvider};
use crate::client::{ConsoleClient, ConsoleError};
use crate::games::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BulkOptions {
    /// Skips the check for an existing cover and overwrites it.
    pub replace_existing: bool,
    pub compression: Compression,
}

/// Reported before each game is handled.
#[derive(Debug, Clone, Copy)]
pub struct BulkProgress<'a> {
    /// Counts from 1.
    pub position: usize,
    pub total: usize,
    pub game: &'a Game,
}

/// Uploads a random cover to each game that has none, like the old app's bulk download with
/// "cover art only". A game's error is recorded and the run goes on; it stops only when
/// reconnecting to the console fails.
pub fn run_bulk_covers<P: CoverProvider>(
    client: &mut ConsoleClient,
    games: &[Game],
    provider: &mut P,
    options: BulkOptions,
    mut on_progress: impl FnMut(BulkProgress<'_>),
) -> Result<BulkReport, BulkError> {
    if games.is_empty() {
        return Err(BulkError::NoGames);
    }
    client.test_connection().map_err(BulkError::Connection)?;

    let mut report = BulkReport {
        entries: Vec::with_capacity(games.len()),
        aborted: None,
    };
    for (index, game) in games.iter().enumerate() {
        on_progress(BulkProgress {
            position: index + 1,
            total: games.len(),
            game,
        });
        if let Err(error) = client.ensure_connected() {
            report.aborted = Some(error);
            break;
        }
        let outcome = handle_game(client, game, provider, options);
        report.entries.push(BulkEntry {
            game: game.clone(),
            outcome,
        });
    }
    Ok(report)
}

fn handle_game<P: CoverProvider>(
    client: &mut ConsoleClient,
    game: &Game,
    provider: &mut P,
    options: BulkOptions,
) -> BulkOutcome {
    if !options.replace_existing
        && let Some(outcome) = check_existing_cover(client, game)
    {
        return outcome;
    }

    let image = match provider.random_cover(&game.ids.title_id) {
        Ok(Some(image)) => image,
        Ok(None) => return BulkOutcome::NoCoverFound,
        Err(error) => return BulkOutcome::Failed(BulkFailure::Provider(Box::new(error))),
    };
    // Not resized: the old app saved the cover at the size it was downloaded.
    let texture = match encode(&image, options.compression) {
        Ok(texture) => texture,
        Err(error) => return BulkOutcome::Failed(BulkFailure::Encode(error)),
    };
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture));
    match client.upload_asset(&game.ids, AssetKind::Cover, &file.to_bytes()) {
        Ok(()) => BulkOutcome::Uploaded,
        Err(error) => BulkOutcome::Failed(BulkFailure::Console(error)),
    }
}

/// `None` means the game has no cover yet and one should be fetched. The texture is not
/// decoded, so a cover in a format the port can't decode still counts as present.
fn check_existing_cover(client: &mut ConsoleClient, game: &Game) -> Option<BulkOutcome> {
    match client.download_asset(&game.ids, AssetKind::Cover) {
        Ok(Some(file)) if file.get(AssetSlot::Cover).is_some() => {
            Some(BulkOutcome::AlreadyHasCover)
        }
        Ok(Some(_) | None) => None,
        Err(error @ ConsoleError::InvalidAsset { .. }) => {
            Some(BulkOutcome::ExistingCoverUnreadable(error))
        }
        Err(error) => Some(BulkOutcome::Failed(BulkFailure::Console(error))),
    }
}
