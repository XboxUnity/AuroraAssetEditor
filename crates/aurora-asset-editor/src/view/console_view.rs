use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::AssetKind;
use aurora_console::{FtpSettings, Game};
use aurora_ui::{ConsoleModel, GameRowView, KindMarks, MainWindow};
use slint::ComponentHandle;

use crate::app::{App, Menu};
use crate::console::{ConsoleLink, ConsoleState, GameColumn, PresenceCell};

use super::GameRows;
use super::shell_view::{Refresh, action_runner};

const NOT_SET_UP_TEXT: &str = "Not set up";
/// The table's sortable columns in the order `ConsoleModel.sort` numbers them.
const COLUMNS: [GameColumn; 5] = [
    GameColumn::Title,
    GameColumn::TitleId,
    GameColumn::DatabaseId,
    GameColumn::MediaId,
    GameColumn::Disc,
];
const NO_ROW: i32 = -1;

/// `KindMarks` values.
const MARK_UNKNOWN: i32 = 0;
const MARK_PRESENT: i32 = 1;
const MARK_ABSENT: i32 = 2;
const MARK_FAILED: i32 = 3;

/// `ConsoleModel.link-state` values.
const LINK_NOT_SET_UP: i32 = 0;
const LINK_UNKNOWN: i32 = 1;
const LINK_CONNECTED: i32 = 2;
const LINK_FAILED: i32 = 3;

/// Connects the Console page's buttons, filters and game table to `app`. The Download, Upload current and
/// Remove menus end in `ShellModel.menu-action`.
pub(super) fn connect_console(window: &MainWindow, app: &Rc<RefCell<App>>, refresh: &Refresh) {
    let console = window.global::<ConsoleModel>();

    let act = action_runner(app, refresh);
    console.on_edit_connection(move || act(&mut |app| app.edit_connection()));

    let act = action_runner(app, refresh);
    console.on_refresh(move || act(&mut |app| app.refresh_games()));

    let act = action_runner(app, refresh);
    console.on_bulk(move || act(&mut |app| app.open_bulk_actions()));

    let act = action_runner(app, refresh);
    console.on_edit_filters(move |title, title_id| {
        act(&mut |app| app.set_game_filter(&title, &title_id));
    });

    let act = action_runner(app, refresh);
    console.on_sort(move |column| {
        if let Some(column) = column_of(column) {
            act(&mut |app| app.sort_games(column));
        }
    });

    let act = action_runner(app, refresh);
    console.on_select(move |row| {
        if let Some(row) = row_of(row) {
            act(&mut |app| app.select_game(row));
        }
    });

    let act = action_runner(app, refresh);
    console.on_open_in_editor(move || act(&mut |app| app.open_in_editor()));

    let act = action_runner(app, refresh);
    console.on_open_game(move |row| {
        if let Some(row) = row_of(row) {
            act(&mut |app| app.open_game_in_editor(row));
        }
    });

    let act = action_runner(app, refresh);
    console.on_move_selection(move |step| {
        let step = isize::try_from(step).unwrap_or_default();
        act(&mut |app| app.move_game_selection(step));
    });

    let act = action_runner(app, refresh);
    console.on_select_first(move || act(&mut |app| app.select_first_game()));

    let act = action_runner(app, refresh);
    console.on_select_last(move || act(&mut |app| app.select_last_game()));
}

pub(super) fn render_console(window: &MainWindow, app: &App, rows: &mut GameRows) {
    let model = window.global::<ConsoleModel>();
    let console = app.console();
    model.set_chip_text(chip_text(console.settings.as_ref()).into());
    model.set_link_state(link_state(&console.link));

    let filter = console.table.filter();
    model.set_filter_title(filter.title.as_str().into());
    model.set_filter_title_id(filter.title_id.as_str().into());

    let visible = console.table.visible();
    let selected_row = console.table.selected_visible_row();
    rows.show(game_rows(console, &visible, selected_row), Clone::clone);
    model.set_selected_row(selected_row.map_or(NO_ROW, row_number));
    model.set_has_selection(selected_row.is_some());

    let sort = console.table.sort();
    let sort_column = sort.and_then(|(column, _)| COLUMNS.iter().position(|each| *each == column));
    model.set_sort_column(sort_column.map_or(NO_ROW, row_number));
    model.set_sort_ascending(sort.is_some_and(|(_, ascending)| ascending));

    let reason = app.console_actions_disabled_reason().unwrap_or_default();
    model.set_actions_reason(reason.into());
    model.set_console_busy(app.locks().console_actions);
    if !matches!(
        app.open_menu(),
        Some(Menu::Download | Menu::Upload | Menu::Remove)
    ) {
        model.set_menu_anchor_x(0.0);
        model.set_menu_anchor_y(0.0);
    }
}

fn chip_text(settings: Option<&FtpSettings>) -> String {
    match settings {
        Some(settings) => format!(
            "{}:{} · {}",
            settings.host, settings.port, settings.username
        ),
        None => NOT_SET_UP_TEXT.to_owned(),
    }
}

fn link_state(link: &ConsoleLink) -> i32 {
    match link {
        ConsoleLink::NotSetUp => LINK_NOT_SET_UP,
        ConsoleLink::Unknown => LINK_UNKNOWN,
        ConsoleLink::Connected { .. } => LINK_CONNECTED,
        ConsoleLink::Failed => LINK_FAILED,
    }
}

fn game_rows(
    console: &ConsoleState,
    visible: &[&Game],
    selected_row: Option<usize>,
) -> Vec<GameRowView> {
    visible
        .iter()
        .enumerate()
        .map(|(row, game)| GameRowView {
            title: game.title_name.as_str().into(),
            title_id: game.ids.title_id.to_string().into(),
            database_id: game.ids.database_id.to_string().into(),
            media_id: game.media_id.to_string().into(),
            disc: game.disc.to_string().into(),
            marks: kind_marks(console.table.presence(&game.ids)),
            selected: selected_row == Some(row),
        })
        .collect()
}

fn kind_marks(cell: PresenceCell) -> KindMarks {
    let mark = |kind| match cell {
        PresenceCell::Unknown => MARK_UNKNOWN,
        PresenceCell::Failed => MARK_FAILED,
        PresenceCell::Known(presence) if presence.has(kind) => MARK_PRESENT,
        PresenceCell::Known(_) => MARK_ABSENT,
    };
    KindMarks {
        cover: mark(AssetKind::Cover),
        background: mark(AssetKind::Background),
        icon_banner: mark(AssetKind::IconBanner),
        screenshots: mark(AssetKind::Screenshots),
    }
}

fn row_number(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

fn column_of(column: i32) -> Option<GameColumn> {
    let sorted = usize::try_from(column)
        .ok()
        .and_then(|index| COLUMNS.get(index))
        .copied();
    if sorted.is_none() {
        log::error!("The window sorted by column {column}, which cannot be sorted");
    }
    sorted
}

fn row_of(row: i32) -> Option<usize> {
    let index = usize::try_from(row).ok();
    if index.is_none() {
        log::error!("The window selected row {row}, which does not exist");
    }
    index
}
