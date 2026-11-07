use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, Row};

use super::Game;
use crate::ids::{GameIds, HexId};

const DEFAULT_DISC: u32 = 1;

#[derive(Debug)]
pub enum ContentDbError {
    Open(rusqlite::Error),
    Query(rusqlite::Error),
    MissingColumn {
        column: &'static str,
        source: rusqlite::Error,
    },
    BadValue {
        row: usize,
        column: &'static str,
        source: rusqlite::Error,
    },
}

impl fmt::Display for ContentDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(_) => write!(f, "cannot open the Content.db file"),
            Self::Query(_) => write!(f, "cannot read the ContentItems table of Content.db"),
            Self::MissingColumn { column, .. } => write!(f, "Content.db has no {column} column"),
            Self::BadValue { row, column, .. } => write!(
                f,
                "Content.db row {row} has an unusable value in its {column} column"
            ),
        }
    }
}

impl std::error::Error for ContentDbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open(source)
            | Self::Query(source)
            | Self::MissingColumn { source, .. }
            | Self::BadValue { source, .. } => Some(source),
        }
    }
}

/// Reads Aurora's game list in table order. Opens read-write without the create flag, so a
/// missing file is an error instead of a new empty database, and a WAL-mode file is read correctly.
pub fn read_content_db(path: &Path) -> Result<Vec<Game>, ContentDbError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags).map_err(ContentDbError::Open)?;
    let mut statement = connection
        .prepare("SELECT * FROM ContentItems")
        .map_err(ContentDbError::Query)?;
    let mut rows = statement.query([]).map_err(ContentDbError::Query)?;
    let mut games = Vec::new();
    while let Some(row) = rows.next().map_err(ContentDbError::Query)? {
        games.push(game_from_row(row, games.len() + 1)?);
    }
    Ok(games)
}

fn game_from_row(row: &Row<'_>, row_number: usize) -> Result<Game, ContentDbError> {
    let whole_number = |column: &'static str| {
        row.get::<_, i64>(column)
            .map_err(|source| column_error(source, row_number, column))
    };
    let database_id = whole_number("Id")?;
    let title_id = whole_number("TitleId")?;
    let media_id = whole_number("MediaId")?;
    let disc = whole_number("DiscNum")?;
    let title_name = row
        .get::<_, Option<String>>("TitleName")
        .map_err(|source| column_error(source, row_number, "TitleName"))?;
    Ok(Game {
        title_name: title_name.unwrap_or_default(),
        ids: GameIds {
            title_id: HexId::from_database_value(title_id),
            database_id: HexId::from_database_value(database_id),
        },
        media_id: HexId::from_database_value(media_id),
        disc: disc_number(disc),
    })
}

fn column_error(source: rusqlite::Error, row: usize, column: &'static str) -> ContentDbError {
    match source {
        rusqlite::Error::InvalidColumnName(_) => ContentDbError::MissingColumn { column, source },
        _ => ContentDbError::BadValue {
            row,
            column,
            source,
        },
    }
}

/// The old app cast the column with `(int)(long)` and treated zero and negative discs as disc 1.
fn disc_number(value: i64) -> u32 {
    let truncated = value as i32;
    if truncated <= 0 {
        DEFAULT_DISC
    } else {
        truncated as u32
    }
}
