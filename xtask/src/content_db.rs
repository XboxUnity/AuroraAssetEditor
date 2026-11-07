//! Sample `Content.db` files for the console reader tests. `Content.db` is Aurora's own SQLite
//! database; the samples carry its `ContentItems` table so the reader meets the console's columns.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

const SAMPLES_DIRECTORY: [&str; 3] = ["fixtures", "console", "content-db"];

/// Aurora's `ContentItems` table (`AuroraSql.cpp`), including its `INTEGET` typo on `ContentGroup`.
/// Columns Aurora leaves unspecified say `NULL` here, which SQLite treats the same way.
const AURORA_CONTENT_ITEMS: &str = "CREATE TABLE IF NOT EXISTS ContentItems (
    Id INTEGER PRIMARY KEY AUTOINCREMENT,
    Directory TEXT COLLATE NOCASE NULL,
    Executable TEXT COLLATE NOCASE NULL,
    TitleId INTEGER NULL,
    MediaId INTEGER NULL,
    BaseVersion INTEGER NULL,
    DiscNum INTEGER NULL,
    DiscsInSet INTEGER NULL,
    TitleName TEXT COLLATE NOCASE NULL,
    Description TEXT COLLATE NOCASE NULL,
    Publisher TEXT COLLATE NOCASE NULL,
    Developer TEXT COLLATE NOCASE NULL,
    LiveRating REAL NULL,
    LiveRaters INTEGER NULL,
    ReleaseDate TEXT COLLATE NOCASE NULL,
    GenreFlag INTEGER NULL,
    ContentFlags INTEGER NULL,
    Hash TEXT COLLATE NOCASE NULL,
    GameCapsOnline INTEGER NULL,
    GameCapsOffline INTEGER NULL,
    GameCapsFlags INTEGER NULL,
    FileType INTEGER NULL,
    ContentType INTEGER NULL,
    ContentGroup INTEGET NULL,
    DefaultGroup INTEGER NULL,
    DateAdded INTEGER NULL,
    FoundAtDepth INTEGER NULL,
    SystemLink INTEGER NULL,
    ScanPathId INTEGER NULL,
    UNIQUE (Directory, Executable, ScanPathId)
)";

const MINIMAL_CONTENT_ITEMS: &str = "CREATE TABLE ContentItems (
    Id INTEGER PRIMARY KEY,
    TitleId INTEGER NULL,
    MediaId INTEGER NULL,
    DiscNum INTEGER NULL,
    TitleName TEXT NULL
)";

const OTHER_TABLE: &str = "CREATE TABLE Other (x INTEGER NULL)";

const INSERT_ROW: &str = "INSERT INTO ContentItems (Id, TitleId, MediaId, DiscNum, TitleName) VALUES (?1, ?2, ?3, ?4, ?5)";

struct Row {
    id: i64,
    title_id: Option<i64>,
    media_id: i64,
    disc_number: i64,
    title_name: Option<&'static str>,
}

const fn row(
    id: i64,
    title_id: i64,
    media_id: i64,
    disc_number: i64,
    title_name: &'static str,
) -> Row {
    Row {
        id,
        title_id: Some(title_id),
        media_id,
        disc_number,
        title_name: Some(title_name),
    }
}

fn typical_rows() -> Vec<Row> {
    vec![
        row(1, 0x4D53_07E6, 0x1A2B_3C4D, 1, "Halo 3"),
        row(2, 0x4D53_0842, 0x0BAD_F00D, 2, "Fable II"),
        row(18, 0x5841_0A1C, 0, 1, "Hexic HD"),
    ]
}

/// Values the old `(int)(long)` casts and the null-title rule have to cope with.
fn edge_rows() -> Vec<Row> {
    vec![
        Row {
            id: 10,
            title_id: Some(-1),
            media_id: -2,
            disc_number: 0,
            title_name: None,
        },
        row(11, 0x1_4D53_07E6, 0x7FFF_FFFF, -3, "Pokémon ポケモン"),
        row(4_294_967_297, 0x4D53_07E6, 1, 4_294_967_298, "  Spaces  "),
    ]
}

fn null_title_id_rows() -> Vec<Row> {
    vec![Row {
        id: 1,
        title_id: None,
        media_id: 1,
        disc_number: 1,
        title_name: Some("Broken"),
    }]
}

fn write_database(path: &Path, create_tables: &[&str], rows: &[Row]) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).with_context(|| format!("cannot replace {}", path.display()))?;
    }
    let connection =
        Connection::open(path).with_context(|| format!("cannot create {}", path.display()))?;
    for statement in create_tables {
        connection.execute(statement, [])?;
    }
    for row in rows {
        connection.execute(
            INSERT_ROW,
            params![
                row.id,
                row.title_id,
                row.media_id,
                row.disc_number,
                row.title_name
            ],
        )?;
    }
    connection
        .close()
        .map_err(|(_, error)| error)
        .with_context(|| format!("cannot finish {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

fn samples_directory() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root");
    SAMPLES_DIRECTORY
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// Rewrites the five sample databases under `fixtures/console/content-db`.
pub fn write_samples() -> Result<()> {
    let directory = samples_directory();
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;
    let aurora = [AURORA_CONTENT_ITEMS];
    write_database(&directory.join("typical.db"), &aurora, &typical_rows())?;
    write_database(&directory.join("edge_values.db"), &aurora, &edge_rows())?;
    write_database(
        &directory.join("minimal_columns.db"),
        &[MINIMAL_CONTENT_ITEMS],
        &[row(5, 0x4D53_07E6, 1, 1, "Halo 3")],
    )?;
    write_database(
        &directory.join("null_title_id.db"),
        &aurora,
        &null_title_id_rows(),
    )?;
    write_database(&directory.join("not_aurora.db"), &[OTHER_TABLE], &[])?;
    Ok(())
}
