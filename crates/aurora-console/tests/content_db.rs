use std::path::PathBuf;

use aurora_console::{ContentDbError, Game, GameFilter, read_content_db};

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/console/content-db")
        .join(name)
}

fn read_sample(name: &str) -> Vec<Game> {
    read_content_db(&sample(name)).expect("the sample database reads")
}

fn filter(title: &str, title_id: &str) -> GameFilter {
    GameFilter {
        title: title.to_string(),
        title_id: title_id.to_string(),
    }
}

#[test]
fn reads_typical_db() {
    let games = read_sample("typical.db");
    assert_eq!(games.len(), 3);
    let halo = &games[0];
    assert_eq!(halo.title_name, "Halo 3");
    assert_eq!(halo.ids.title_id.as_str(), "4D5307E6");
    assert_eq!(halo.ids.database_id.as_str(), "00000001");
    assert_eq!(halo.media_id.as_str(), "1A2B3C4D");
    assert_eq!(halo.disc, 1);
    assert_eq!(games[1].title_name, "Fable II");
    assert_eq!(games[1].disc, 2);
    assert_eq!(games[2].ids.database_id.as_str(), "00000012");
}

#[test]
fn reads_edge_values() {
    let games = read_sample("edge_values.db");
    let title_ids: Vec<&str> = games
        .iter()
        .map(|game| game.ids.title_id.as_str())
        .collect();
    let media_ids: Vec<&str> = games.iter().map(|game| game.media_id.as_str()).collect();
    let discs: Vec<u32> = games.iter().map(|game| game.disc).collect();
    let database_ids: Vec<&str> = games
        .iter()
        .map(|game| game.ids.database_id.as_str())
        .collect();
    let titles: Vec<&str> = games.iter().map(|game| game.title_name.as_str()).collect();
    assert_eq!(title_ids, ["FFFFFFFF", "4D5307E6", "4D5307E6"]);
    assert_eq!(media_ids, ["FFFFFFFE", "7FFFFFFF", "00000001"]);
    assert_eq!(discs, [1, 1, 2]);
    assert_eq!(database_ids, ["0000000A", "0000000B", "00000001"]);
    assert_eq!(titles, ["", "Pokémon ポケモン", "  Spaces  "]);
}

#[test]
fn minimal_columns_db_is_enough() {
    let games = read_sample("minimal_columns.db");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].title_name, "Halo 3");
    assert_eq!(games[0].ids.title_id.as_str(), "4D5307E6");
    assert_eq!(games[0].ids.database_id.as_str(), "00000005");
    assert_eq!(games[0].media_id.as_str(), "00000001");
    assert_eq!(games[0].disc, 1);
}

#[test]
fn null_title_id_names_row_and_column() {
    let error = read_content_db(&sample("null_title_id.db")).unwrap_err();
    assert!(matches!(
        error,
        ContentDbError::BadValue {
            row: 1,
            column: "TitleId",
            ..
        }
    ));
    let message = error.to_string();
    assert!(message.contains("TitleId"), "{message}");
    assert!(message.contains("row 1"), "{message}");
}

#[test]
fn missing_table_is_query_error() {
    let error = read_content_db(&sample("not_aurora.db")).unwrap_err();
    assert!(matches!(error, ContentDbError::Query(_)), "{error:?}");
}

#[test]
fn empty_or_whitespace_filter_matches_all() {
    let games = read_sample("typical.db");
    for (title, title_id) in [("", ""), ("  ", ""), ("", "  "), ("  ", "  ")] {
        let filter = filter(title, title_id);
        assert!(games.iter().all(|game| filter.matches(game)));
    }
}

#[test]
fn title_and_title_id_are_anded() {
    let halo = &read_sample("typical.db")[0];
    assert!(filter("halo", "07e6").matches(halo));
    assert!(!filter("halo", "0842").matches(halo));
}

#[test]
fn filter_folds_non_ascii_case() {
    let games = read_sample("edge_values.db");
    assert!(filter("POKÉMON", "").matches(&games[1]));
}

#[test]
fn filter_is_not_trimmed() {
    let halo = &read_sample("typical.db")[0];
    assert!(!filter(" halo", "").matches(halo));
}

#[test]
fn missing_file_is_open_error_and_creates_nothing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("Content.db");
    let error = read_content_db(&path).unwrap_err();
    assert!(matches!(error, ContentDbError::Open(_)), "{error:?}");
    assert!(!path.exists());
}

#[test]
fn wal_mode_db_reads_correctly() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("Content.db");
    let connection = rusqlite::Connection::open(&path).expect("create database");
    connection
        .query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))
        .expect("enable WAL");
    connection
        .execute_batch(
            "CREATE TABLE ContentItems (Id INTEGER PRIMARY KEY, TitleId INTEGER NULL, \
             MediaId INTEGER NULL, DiscNum INTEGER NULL, TitleName TEXT NULL);
             INSERT INTO ContentItems VALUES (7, 1297287142, 3, 2, 'Halo 3');",
        )
        .expect("insert row");
    connection.close().expect("close database");
    let games = read_content_db(&path).expect("WAL database reads");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].ids.title_id.as_str(), "4D5307E6");
    assert_eq!(games[0].ids.database_id.as_str(), "00000007");
    assert_eq!(games[0].disc, 2);
}

#[test]
fn missing_column_is_named_not_a_bad_value() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("Content.db");
    let connection = rusqlite::Connection::open(&path).expect("create database");
    connection
        .execute_batch(
            "CREATE TABLE ContentItems (Id INTEGER PRIMARY KEY, TitleId INTEGER NULL, \
             MediaId INTEGER NULL, TitleName TEXT NULL);
             INSERT INTO ContentItems VALUES (1, 1, 1, 'Halo 3');",
        )
        .expect("insert row");
    connection.close().expect("close database");
    let error = read_content_db(&path).unwrap_err();
    assert!(
        matches!(
            error,
            ContentDbError::MissingColumn {
                column: "DiscNum",
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(error.to_string(), "Content.db has no DiscNum column");
}
