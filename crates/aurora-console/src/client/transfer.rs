use aurora_asset::{AssetFile, AssetKind};

use super::{AssetPresence, ConsoleClient, ConsoleError, ConsoleStatus};
use crate::ftp::{ListingEntry, find_entry, parse_listing};
use crate::ids::GameIds;
use crate::paths::{asset_file_name, asset_folder};

pub(crate) const MAX_LISTING_BYTES: u64 = 64 * 1024 * 1024;
/// 20 uncompressed 1000x562 screenshots are about 47 MB.
pub(crate) const MAX_ASSET_FILE_BYTES: u64 = 256 * 1024 * 1024;
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
        let listing_limit = self.listing_limit;
        let connection = self.connection(&format!("list {path}"))?;
        let lines = if connection.features().mlsd {
            connection.mlsd(path, listing_limit)?
        } else {
            connection.list(path, listing_limit)?
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use test_servers::{FakeConsole, FakeConsoleConfig};

    use super::*;
    use crate::{FtpSettings, Timeouts};

    const SMALL_LISTING_LIMIT: u64 = 100;

    #[test]
    fn listing_longer_than_the_limit_is_an_error() {
        let console = FakeConsole::start(FakeConsoleConfig::default());
        for number in 0..10 {
            console.add_file(&format!("/Game/Data/DataBases/file{number}.bin"), &[1]);
        }
        let timeout = Duration::from_millis(500);
        let mut client = ConsoleClient::new(
            FtpSettings {
                host: "127.0.0.1".to_owned(),
                port: console.port().to_string(),
                username: "xboxftp".to_owned(),
                password: "xboxftp".to_owned(),
            },
            Timeouts {
                connect: timeout,
                read: timeout,
            },
            |_| {},
        );
        client.ensure_connected().unwrap();
        client.listing_limit = SMALL_LISTING_LIMIT;

        let error = client.list_folder("/Game/Data/DataBases").unwrap_err();

        assert!(matches!(
            error,
            ConsoleError::ListingTooLarge {
                limit: SMALL_LISTING_LIMIT,
                ..
            }
        ));
    }
}
