use std::fs;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetFile, AssetKind, AssetSlot, Compression, RgbaImage, encode};
use aurora_console::{AssetPresence, GameIds, HexId};
use editor_test_support::{TEST_ASSETS_VARIABLE, TestAssets, TestAssetsError};
use tempfile::TempDir;

const TITLE_ID: &str = "4D5307E6";
const GAME_FOLDER: &str = "4D5307E6_00000001";
const COVER_FILE: &str = "GC4D5307E6.asset";
const BACKGROUND_FILE: &str = "BK4D5307E6.asset";

/// An asset folder in a temporary folder: `typical.db` as `DataBases/content.db` and no game files.
struct Fixture {
    folder: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let folder = tempfile::tempdir().expect("a temporary folder is needed for the assets");
        let databases = folder.path().join("DataBases");
        fs::create_dir_all(&databases).expect("the DataBases folder is created");
        fs::create_dir_all(folder.path().join("GameData").join(GAME_FOLDER))
            .expect("the game folder is created");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/console/content-db/typical.db"),
            databases.join("content.db"),
        )
        .expect("the content database fixture is copied");
        Fixture { folder }
    }

    fn assets(&self) -> TestAssets {
        TestAssets::from_value(self.folder.path().to_str()).expect("the folder is a valid one")
    }

    fn game_file(&self, name: &str) -> PathBuf {
        self.folder
            .path()
            .join("GameData")
            .join(GAME_FOLDER)
            .join(name)
    }

    fn write_cover(&self) {
        let cover = encode(&RgbaImage::new(8, 8), Compression::None)
            .expect("an 8x8 image is within the encodable size range");
        let mut file = AssetFile::new();
        file.set(AssetSlot::Cover, Some(cover));
        self.write_asset(COVER_FILE, &mut file);
    }

    fn write_empty_background(&self) {
        self.write_asset(BACKGROUND_FILE, &mut AssetFile::new());
    }

    fn write_asset(&self, name: &str, file: &mut AssetFile) {
        fs::write(self.game_file(name), file.to_bytes()).expect("the asset file is written");
    }
}

fn ids() -> GameIds {
    GameIds {
        title_id: HexId::parse(TITLE_ID).expect("the title id is valid"),
        database_id: HexId::parse("00000001").expect("the database id is valid"),
    }
}

fn error_of<T>(result: Result<T, TestAssetsError>) -> TestAssetsError {
    match result {
        Ok(_) => panic!("the call should have failed"),
        Err(error) => error,
    }
}

#[test]
fn unset_variable_names_the_variable_and_the_layout() {
    for value in [None, Some("")] {
        let error = error_of(TestAssets::from_value(value));
        assert!(matches!(error, TestAssetsError::NotSet));
        let message = error.to_string();
        assert!(message.contains(TEST_ASSETS_VARIABLE), "{message}");
        assert!(message.contains("GameData"), "{message}");
        assert!(message.contains("DataBases/content.db"), "{message}");
    }
}

#[test]
fn a_path_that_is_not_a_folder_is_refused() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed");
    let file = folder.path().join("a-file.txt");
    fs::write(&file, b"not a folder").expect("the file is written");

    let error = error_of(TestAssets::from_value(file.to_str()));

    assert!(matches!(&error, TestAssetsError::NotAFolder(path) if *path == file));
    let message = error.to_string();
    assert!(message.contains(&file.display().to_string()), "{message}");
    assert!(message.contains(TEST_ASSETS_VARIABLE), "{message}");
}

#[test]
fn a_folder_without_game_data_or_content_db_is_refused() {
    let folder = tempfile::tempdir().expect("a temporary folder is needed");

    let error = error_of(TestAssets::from_value(folder.path().to_str()));

    match &error {
        TestAssetsError::MissingParts { parts, .. } => {
            assert_eq!(parts, &vec!["GameData", "DataBases/content.db"]);
        }
        other => panic!("expected MissingParts, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("GameData"), "{message}");
    assert!(message.contains("DataBases/content.db"), "{message}");
    assert!(message.contains(TEST_ASSETS_VARIABLE), "{message}");
}

#[test]
fn games_come_from_content_db() {
    let games = Fixture::new().assets().games().expect("the games are read");

    let titles: Vec<&str> = games.iter().map(|game| game.title_name.as_str()).collect();
    assert_eq!(titles, ["Halo 3", "Fable II", "Hexic HD"]);
    assert_eq!(games[2].ids.title_id.to_string(), "58410A1C");
    assert_eq!(games[2].ids.database_id.to_string(), "00000012");
}

#[test]
fn unreadable_content_db_is_reported_by_its_own_path() {
    let fixture = Fixture::new();
    let database = fixture.folder.path().join("DataBases/content.db");
    fs::write(&database, b"not a database").expect("the database is overwritten");

    let error = error_of(fixture.assets().games());

    assert!(
        matches!(&error, TestAssetsError::Unreadable { path, .. } if *path == database),
        "{error}"
    );
}

#[test]
fn games_are_read_from_a_copy_of_content_db() {
    let fixture = Fixture::new();
    let database = fixture.folder.path().join("DataBases/content.db");
    let before = fs::read(&database).expect("the database is readable");

    fixture.assets().games().expect("the games are read");

    let names: Vec<String> = fs::read_dir(fixture.folder.path().join("DataBases"))
        .expect("the DataBases folder is listed")
        .map(|entry| {
            entry
                .expect("the entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["content.db"]);
    assert_eq!(
        fs::read(&database).expect("the database is readable"),
        before
    );
}

#[test]
fn asset_file_reads_the_game_folder() {
    let fixture = Fixture::new();
    fixture.write_cover();

    let file = fixture
        .assets()
        .asset_file(&ids(), AssetKind::Cover)
        .expect("the cover file is read");

    assert!(file.get(AssetSlot::Cover).is_some());
}

#[test]
fn missing_asset_file_names_the_file_and_the_variable() {
    let fixture = Fixture::new();

    let error = error_of(fixture.assets().asset_file(&ids(), AssetKind::Background));

    assert!(matches!(error, TestAssetsError::Unreadable { .. }));
    let message = error.to_string();
    assert!(message.contains(BACKGROUND_FILE), "{message}");
    assert!(message.contains(GAME_FOLDER), "{message}");
    assert!(message.contains(TEST_ASSETS_VARIABLE), "{message}");
}

#[test]
fn unparsable_asset_file_names_the_reason() {
    let fixture = Fixture::new();
    fs::write(fixture.game_file(COVER_FILE), vec![0u8; 2048]).expect("the file is written");

    let error = error_of(fixture.assets().asset_file(&ids(), AssetKind::Cover));

    assert!(matches!(error, TestAssetsError::Unreadable { .. }));
    let message = error.to_string();
    assert!(message.contains(COVER_FILE), "{message}");
    assert!(message.contains("magic"), "{message}");
}

#[test]
fn texture_of_an_empty_slot_is_an_error() {
    let fixture = Fixture::new();
    fixture.write_empty_background();

    let error = error_of(fixture.assets().texture(
        &ids(),
        AssetKind::Background,
        AssetSlot::Background,
    ));

    assert!(matches!(error, TestAssetsError::EmptySlot { slot: 4, .. }));
    assert!(error.to_string().contains(BACKGROUND_FILE), "{error}");
}

#[test]
fn texture_of_a_filled_slot_is_returned() {
    let fixture = Fixture::new();
    fixture.write_cover();

    let texture = fixture
        .assets()
        .texture(&ids(), AssetKind::Cover, AssetSlot::Cover)
        .expect("the cover texture is read");

    let expected = encode(&RgbaImage::new(8, 8), Compression::None)
        .expect("an 8x8 image is within the encodable size range");
    assert_eq!(texture, expected);
}

#[test]
fn presence_lists_the_kinds_with_a_texture() {
    let fixture = Fixture::new();
    fixture.write_cover();
    fixture.write_empty_background();

    let presence = fixture
        .assets()
        .presence(&ids())
        .expect("the presence is read");

    assert_eq!(
        presence,
        AssetPresence {
            cover: true,
            ..AssetPresence::NONE
        }
    );
}

#[test]
fn presence_reports_an_unparsable_file_as_an_error() {
    let fixture = Fixture::new();
    fs::write(fixture.game_file(COVER_FILE), vec![0u8; 2048]).expect("the file is written");

    let error = error_of(fixture.assets().presence(&ids()));

    assert!(matches!(error, TestAssetsError::Unreadable { .. }));
}
