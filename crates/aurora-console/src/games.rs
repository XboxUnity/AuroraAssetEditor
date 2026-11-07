use std::fmt;
use std::path::Path;

use rusqlite::{Connection, Row};

use crate::ids::{GameIds, HexId};

const DEFAULT_DISC: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub title_name: String,
    pub ids: GameIds,
    pub media_id: HexId,
    pub disc: u32,
}

#[derive(Debug)]
pub enum ContentDbError {
    Open(rusqlite::Error),
    Query(rusqlite::Error),
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
            Self::BadValue { row, column, .. } => write!(
                f,
                "Content.db row {row} has no whole number in its {column} column"
            ),
        }
    }
}

impl std::error::Error for ContentDbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open(source) | Self::Query(source) | Self::BadValue { source, .. } => {
                Some(source)
            }
        }
    }
}

/// Reads Aurora's game list. Like the old app, opens with the default flags and returns the rows in table order.
pub fn read_content_db(path: &Path) -> Result<Vec<Game>, ContentDbError> {
    let connection = Connection::open(path).map_err(ContentDbError::Open)?;
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
            .map_err(|source| ContentDbError::BadValue {
                row: row_number,
                column,
                source,
            })
    };
    let database_id = whole_number("Id")?;
    let title_id = whole_number("TitleId")?;
    let media_id = whole_number("MediaId")?;
    let disc = whole_number("DiscNum")?;
    let title_name = row
        .get::<_, Option<String>>("TitleName")
        .map_err(|source| ContentDbError::BadValue {
            row: row_number,
            column: "TitleName",
            source,
        })?;
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

/// The old app cast the column with `(int)(long)` and treated zero and negative discs as disc 1.
fn disc_number(value: i64) -> u32 {
    let truncated = value as i32;
    if truncated <= 0 {
        DEFAULT_DISC
    } else {
        truncated as u32
    }
}

/// Mirrors `FtpAssetsControl.FiltersChanged`: blank filters are ignored, the others are ANDed
/// case-insensitive substring matches on the untrimmed text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameFilter {
    pub title: String,
    pub title_id: String,
}

impl GameFilter {
    pub fn matches(&self, game: &Game) -> bool {
        contains_unless_blank(&game.title_name, &self.title)
            && contains_unless_blank(game.ids.title_id.as_str(), &self.title_id)
    }
}

fn contains_unless_blank(text: &str, filter: &str) -> bool {
    filter.trim().is_empty() || text.to_lowercase().contains(&filter.to_lowercase())
}
