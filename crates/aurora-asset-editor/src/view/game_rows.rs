use aurora_ui::GameRowView;

use super::KeptRows;

/// The console's game table. Its rows change in place, so a render keeps the part of a row the table is
/// scrolled by and a click in progress on a row.
pub type GameRows = KeptRows<GameRowView, GameRowView>;
