use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetFile, AssetKind, AssetSlot, EncodedTexture};
use aurora_console::{AssetPresence, Game, GameIds, asset_file_name, read_content_db};
use tempfile::TempDir;

pub const TEST_ASSETS_VARIABLE: &str = "AURORA_TEST_ASSETS";

// The layout of a folder pulled from a console. `aurora_console`'s GAME_DATA_DIR, DATABASES_DIR
// and CONTENT_DB_FILE are not reused: they are absolute FTP paths, and the console's `Content.db`
// differs in case from the pulled folder's `content.db`.
const GAME_DATA_FOLDER: &str = "GameData";
const DATABASES_FOLDER: &str = "DataBases";
const CONTENT_DB_FILE_NAME: &str = "content.db";
const CONTENT_DB_PART: &str = "DataBases/content.db";

/// The user's real Aurora assets, pulled from a console into a folder kept outside the
/// repository. Only the README screenshots read them; the tests use generated art.
#[derive(Debug)]
pub struct TestAssets {
    folder: PathBuf,
}

impl TestAssets {
    pub fn from_environment() -> Result<TestAssets, TestAssetsError> {
        TestAssets::from_value(std::env::var(TEST_ASSETS_VARIABLE).ok().as_deref())
    }

    /// An unset or empty `value` is `NotSet`. Every missing part is collected before failing, so
    /// one message lists them all.
    pub fn from_value(value: Option<&str>) -> Result<TestAssets, TestAssetsError> {
        let folder = match value {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => return Err(TestAssetsError::NotSet),
        };
        if !folder.is_dir() {
            return Err(TestAssetsError::NotAFolder(folder));
        }
        let mut parts = Vec::new();
        if !folder.join(GAME_DATA_FOLDER).is_dir() {
            parts.push(GAME_DATA_FOLDER);
        }
        if !content_db_path(&folder).is_file() {
            parts.push(CONTENT_DB_PART);
        }
        if !parts.is_empty() {
            return Err(TestAssetsError::MissingParts { folder, parts });
        }
        Ok(TestAssets { folder })
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The game list of the pulled `content.db`. Read from a copy: opening a database can create
    /// journal files next to it, and the assets folder is never written to.
    pub fn games(&self) -> Result<Vec<Game>, TestAssetsError> {
        let (_copy_folder, copy) = self.content_db_copy()?;
        read_content_db(&copy).map_err(|error| TestAssetsError::Unreadable {
            path: copy,
            reason: error.to_string(),
        })
    }

    pub fn asset_bytes(&self, ids: &GameIds, kind: AssetKind) -> Result<Vec<u8>, TestAssetsError> {
        let path = self.asset_path(ids, kind);
        fs::read(&path).map_err(|error| unreadable(path, &error))
    }

    pub fn asset_file(&self, ids: &GameIds, kind: AssetKind) -> Result<AssetFile, TestAssetsError> {
        let bytes = self.asset_bytes(ids, kind)?;
        AssetFile::parse(&bytes).map_err(|error| unreadable(self.asset_path(ids, kind), &error))
    }

    pub fn texture(
        &self,
        ids: &GameIds,
        kind: AssetKind,
        slot: AssetSlot,
    ) -> Result<EncodedTexture, TestAssetsError> {
        let file = self.asset_file(ids, kind)?;
        match file.get(slot) {
            Some(texture) => Ok(texture.clone()),
            None => Err(TestAssetsError::EmptySlot {
                path: self.asset_path(ids, kind),
                slot: slot.index(),
            }),
        }
    }

    /// A kind is present when its file exists and holds a texture in one of the kind's slots. A
    /// file that is not there is absent; any other failure is an error.
    ///
    /// This differs from the console's check by design: it reads the file and looks for a
    /// texture, where `ConsoleClient::asset_presence` only compares file sizes with the empty
    /// asset's.
    pub fn presence(&self, ids: &GameIds) -> Result<AssetPresence, TestAssetsError> {
        let has = |kind: AssetKind| -> Result<bool, TestAssetsError> {
            let path = self.asset_path(ids, kind);
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(unreadable(path, &error)),
            };
            let file = AssetFile::parse(&bytes).map_err(|error| unreadable(path, &error))?;
            Ok(file.has_entries_for(kind))
        };
        Ok(AssetPresence {
            cover: has(AssetKind::Cover)?,
            background: has(AssetKind::Background)?,
            icon_banner: has(AssetKind::IconBanner)?,
            screenshots: has(AssetKind::Screenshots)?,
        })
    }

    fn asset_path(&self, ids: &GameIds, kind: AssetKind) -> PathBuf {
        self.folder
            .join(GAME_DATA_FOLDER)
            .join(ids.folder_name())
            .join(asset_file_name(kind, &ids.title_id))
    }

    fn content_db_copy(&self) -> Result<(TempDir, PathBuf), TestAssetsError> {
        let source = content_db_path(&self.folder);
        let copy_folder =
            tempfile::tempdir().map_err(|error| unreadable(source.clone(), &error))?;
        let copy = copy_folder.path().join(CONTENT_DB_FILE_NAME);
        fs::copy(&source, &copy).map_err(|error| unreadable(source, &error))?;
        Ok((copy_folder, copy))
    }
}

fn content_db_path(folder: &Path) -> PathBuf {
    folder.join(DATABASES_FOLDER).join(CONTENT_DB_FILE_NAME)
}

fn unreadable(path: PathBuf, reason: &dyn fmt::Display) -> TestAssetsError {
    TestAssetsError::Unreadable {
        path,
        reason: reason.to_string(),
    }
}

#[derive(Debug)]
pub enum TestAssetsError {
    NotSet,
    NotAFolder(PathBuf),
    MissingParts {
        folder: PathBuf,
        parts: Vec<&'static str>,
    },
    Unreadable {
        path: PathBuf,
        reason: String,
    },
    EmptySlot {
        path: PathBuf,
        slot: usize,
    },
}

impl fmt::Display for TestAssetsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSet => write!(
                f,
                "{TEST_ASSETS_VARIABLE} is not set. The README screenshots show real game art from \
                 a folder kept outside the repository: set {TEST_ASSETS_VARIABLE} to a folder \
                 holding GameData/<TitleID>_<DatabaseID>/ and DataBases/content.db, pulled from \
                 an Aurora console."
            ),
            Self::NotAFolder(path) => write!(
                f,
                "{TEST_ASSETS_VARIABLE} points to {}, which is not a folder",
                path.display()
            ),
            Self::MissingParts { folder, parts } => write!(
                f,
                "the folder {} named by {TEST_ASSETS_VARIABLE} is missing {}",
                folder.display(),
                parts.join(" and ")
            ),
            Self::Unreadable { path, reason } => write!(
                f,
                "{} could not be read ({reason}); it belongs to the folder named by \
                 {TEST_ASSETS_VARIABLE}",
                path.display()
            ),
            Self::EmptySlot { path, slot } => write!(
                f,
                "{} holds no texture in entry {slot}; the folder named by {TEST_ASSETS_VARIABLE} \
                 must hold the game's real art",
                path.display()
            ),
        }
    }
}

impl std::error::Error for TestAssetsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_db_is_read_from_outside_the_asset_folder() {
        let folder = tempfile::tempdir().expect("a temporary folder is needed");
        let databases = folder.path().join(DATABASES_FOLDER);
        fs::create_dir_all(&databases).expect("the DataBases folder is created");
        fs::create_dir(folder.path().join(GAME_DATA_FOLDER)).expect("GameData is created");
        let original = databases.join(CONTENT_DB_FILE_NAME);
        fs::write(&original, b"database bytes").expect("the database is written");
        let assets = TestAssets::from_value(folder.path().to_str()).expect("the folder is valid");

        let (_copy_folder, copy) = assets.content_db_copy().expect("the copy is made");

        assert!(!copy.starts_with(assets.folder()));
        assert_eq!(
            fs::read(&copy).expect("the copy is readable"),
            fs::read(&original).expect("the original is readable")
        );
    }
}
