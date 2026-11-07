use aurora_asset::{AssetFile, AssetKind};

use super::{AssetPresence, ConsoleClient, ConsoleError, ConsoleStatus};
use crate::ftp::{ListingEntry, find_entry, parse_listing};
use crate::ids::GameIds;
use crate::paths::{asset_file_name, asset_folder};

/// 20 uncompressed 1000x562 screenshots are about 47 MB.
pub(crate) const MAX_ASSET_FILE_BYTES: u64 = 256 * 1024 * 1024;
#[expect(dead_code, reason = "the Content.db download uses it in the next task")]
pub(crate) const MAX_CONTENT_DB_BYTES: u64 = 512 * 1024 * 1024;

impl ConsoleClient {
    /// `Ok(None)` when the game has no such asset: no folder, no file, or an empty one.
    pub fn download_asset(
        &mut self,
        ids: &GameIds,
        kind: AssetKind,
    ) -> Result<Option<AssetFile>, ConsoleError> {
        let folder = asset_folder(ids);
        let file = asset_file_name(kind, &ids.title_id);
        self.run_operation(|client| {
            let bytes = match client.download_listed(&folder, &file, MAX_ASSET_FILE_BYTES) {
                Ok(Some(bytes)) => bytes,
                Ok(None) | Err(ConsoleError::MissingFolder { .. }) => return Ok(None),
                Err(other) => return Err(other),
            };
            AssetFile::parse(&bytes)
                .map(Some)
                .map_err(|source| ConsoleError::InvalidAsset { file, source })
        })
    }

    /// Replaces the game's asset file of this kind; the game's folder must already exist.
    pub fn upload_asset(
        &mut self,
        ids: &GameIds,
        kind: AssetKind,
        file_bytes: &[u8],
    ) -> Result<(), ConsoleError> {
        let folder = asset_folder(ids);
        let file = asset_file_name(kind, &ids.title_id);
        self.run_operation(|client| {
            client.open_folder(&folder)?;
            client.report(ConsoleStatus::Uploading { file: file.clone() });
            client
                .connection(&format!("upload {file}"))?
                .store(&file, file_bytes)
        })
    }

    /// Uploads the empty asset, as the old app's Remove did.
    pub fn remove_asset(&mut self, ids: &GameIds, kind: AssetKind) -> Result<(), ConsoleError> {
        self.upload_asset(ids, kind, &empty_asset_bytes())
    }

    /// A kind counts as present only when its file is bigger than the empty asset.
    pub fn asset_presence(&mut self, ids: &GameIds) -> Result<AssetPresence, ConsoleError> {
        let folder = asset_folder(ids);
        self.run_operation(|client| {
            let entries = match client.list_folder(&folder) {
                Ok(entries) => entries,
                Err(ConsoleError::MissingFolder { .. }) => return Ok(AssetPresence::NONE),
                Err(other) => return Err(other),
            };
            let empty_asset_len = empty_asset_bytes().len() as u64;
            let has = |kind| {
                is_present(
                    &entries,
                    &asset_file_name(kind, &ids.title_id),
                    empty_asset_len,
                )
            };
            Ok(AssetPresence {
                cover: has(AssetKind::Cover),
                background: has(AssetKind::Background),
                icon_banner: has(AssetKind::IconBanner),
                screenshots: has(AssetKind::Screenshots),
            })
        })
    }

    pub(crate) fn open_folder(&mut self, path: &str) -> Result<(), ConsoleError> {
        self.report(ConsoleStatus::OpeningFolder {
            path: path.to_owned(),
        });
        let reply = self.execute(&format!("CWD {path}"))?;
        match reply.code / 100 {
            1..=3 => Ok(()),
            4 => Err(ConsoleError::Refused {
                action: format!("open {path}"),
                reply: format!("{} {}", reply.code, reply.message),
            }),
            _ => Err(ConsoleError::MissingFolder {
                path: path.to_owned(),
            }),
        }
    }

    pub(crate) fn list_folder(&mut self, path: &str) -> Result<Vec<ListingEntry>, ConsoleError> {
        self.open_folder(path)?;
        let connection = self.connection(&format!("list {path}"))?;
        let lines = if connection.features().mlsd {
            connection.mlsd(path)?
        } else {
            connection.list(path)?
        };
        Ok(parse_listing(&lines))
    }

    /// The size comes from the folder listing, since the transfer must deliver exactly that many
    /// bytes. `None` when the name is not listed or is listed empty.
    pub(crate) fn download_listed(
        &mut self,
        folder: &str,
        name: &str,
        limit: u64,
    ) -> Result<Option<Vec<u8>>, ConsoleError> {
        let entries = self.list_folder(folder)?;
        let Some(entry) = find_entry(&entries, name) else {
            return Ok(None);
        };
        let size = match entry.size {
            Some(size) if size > 0 => size,
            _ => return Ok(None),
        };
        if size > limit {
            return Err(ConsoleError::FileTooLarge {
                file: entry.name.clone(),
                size,
                limit,
            });
        }
        self.report(ConsoleStatus::Downloading {
            file: entry.name.clone(),
        });
        self.connection(&format!("download {}", entry.name))?
            .retrieve(&entry.name, size)
            .map(Some)
    }
}

fn empty_asset_bytes() -> Vec<u8> {
    AssetFile::new().to_bytes()
}

fn is_present(entries: &[ListingEntry], file_name: &str, empty_asset_len: u64) -> bool {
    find_entry(entries, file_name)
        .and_then(|entry| entry.size)
        .is_some_and(|size| size > empty_asset_len)
}
