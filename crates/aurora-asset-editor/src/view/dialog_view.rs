use std::cell::RefCell;
use std::rc::Rc;

use aurora_console::Game;
use aurora_online::CoverSource;
use aurora_ui::{ChoiceView, DialogKind, DialogModel, MainWindow, PickerGameView};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::about::{BuildInfo, notices_text};
use crate::app::{ABOUT_TITLE, App, BULK_TITLE, PickerContent, REFRESHING_TEXT};
use crate::dialogs::{BulkForm, ChoiceId, ConnectionForm, Modal, ModalAnswer, ModalField};
use crate::status::Severity;

use super::PickerRows;
use super::shell_view::{Refresh, action_runner};

/// The choice dialog draws a button for each of the app's choices, at most two (Icon or Banner, Replace or
/// Add to free slots).
const CHOICE_BUTTONS: usize = 2;
const CONNECTION_TITLE: &str = "Console connection";
const NO_ROW: i32 = -1;
const APP_NAME: &str = "Aurora Asset Editor";
const ABOUT_CREDITS: &str = "by Swizzy, thanks to MaesterRowen";
const BULK_SOURCES: [CoverSource; 2] = [CoverSource::XboxUnity, CoverSource::InternetArchive];
const TITLE_ID_FIELDS: [ModalField; 1] = [ModalField::TitleId];
const UPLOAD_TARGET_FIELDS: [ModalField; 2] =
    [ModalField::UploadTitleId, ModalField::UploadDatabaseId];
const CONNECTION_FIELDS: [ModalField; 4] = [
    ModalField::Host,
    ModalField::Port,
    ModalField::Username,
    ModalField::Password,
];

/// Connects the dialogs' buttons, boxes and keys to `app`.
pub(super) fn connect_dialogs(window: &MainWindow, app: &Rc<RefCell<App>>, refresh: &Refresh) {
    let dialog = window.global::<DialogModel>();

    let act = action_runner(app, refresh);
    dialog.on_accept(move || act(&mut |app| app.answer_modal(ModalAnswer::Accept)));

    let act = action_runner(app, refresh);
    dialog.on_cancel(move || act(&mut |app| app.answer_modal(ModalAnswer::Cancel)));

    let act = action_runner(app, refresh);
    dialog.on_choose(move |index| {
        act(&mut |app| {
            if let Some(choice) = offered_choice(app.modal(), index) {
                app.answer_modal(ModalAnswer::Choice(choice));
            }
        });
    });

    let act = action_runner(app, refresh);
    dialog.on_edit_field(move |index, text| {
        act(&mut |app| {
            if let Some(field) = form_field(app.modal(), index) {
                app.edit_modal_field(field, &text);
            }
        });
    });

    let act = action_runner(app, refresh);
    dialog.on_edit_picker_filter(move |text| act(&mut |app| app.edit_picker_filter(&text)));

    let act = action_runner(app, refresh);
    dialog.on_move_picker_highlight(move |step| {
        let step = isize::try_from(step).unwrap_or_default();
        act(&mut |app| app.move_picker_highlight(step));
    });

    let act = action_runner(app, refresh);
    dialog.on_pick_game(move |index| match usize::try_from(index) {
        Ok(index) => act(&mut |app| app.pick_game(index)),
        Err(_) => log::error!("The window picked game {index}, which the picker does not list"),
    });

    let act = action_runner(app, refresh);
    dialog.on_pick_highlighted_game(move || act(&mut |app| app.pick_highlighted_game()));

    let act = action_runner(app, refresh);
    dialog.on_test_connection(move || act(&mut |app| app.test_connection()));

    let act = action_runner(app, refresh);
    dialog.on_set_bulk_source(move |index| {
        let source = usize::try_from(index)
            .ok()
            .and_then(|index| BULK_SOURCES.get(index));
        match source {
            Some(&source) => act(&mut |app| app.set_bulk_source(source)),
            None => {
                log::error!("The window chose bulk source {index}, which the dialog does not offer")
            }
        }
    });

    let act = action_runner(app, refresh);
    dialog.on_set_bulk_replace(move |replace| act(&mut |app| app.set_bulk_replace(replace)));

    let act = action_runner(app, refresh);
    dialog.on_toggle_notices(move || act(&mut |app| app.toggle_about_notices()));

    let act = action_runner(app, refresh);
    dialog.on_copy_build_info(move || act(&mut |app| app.copy_build_info()));

    let act = action_runner(app, refresh);
    dialog.on_open_log_folder(move || act(&mut |app| app.open_log_folder()));
}

pub(super) fn render_dialogs(window: &MainWindow, app: &App, picker_rows: &mut PickerRows) {
    let content = DialogContent::of(app.modal(), app.build_info());
    let dialog = window.global::<DialogModel>();
    dialog.set_kind(content.kind);
    dialog.set_title(content.title.into());
    dialog.set_text(content.text.into());
    dialog.set_confirm_label(content.confirm_label.into());
    dialog.set_danger(content.danger);
    // A new model on every render is safe here: the dialogs read choices and fields by index, never with
    // `for`, so their buttons and boxes stay in place, with any press, caret or focus they hold.
    dialog.set_choices(ModelRc::new(VecModel::from(content.choices)));
    dialog.set_fields(ModelRc::new(VecModel::from(content.fields)));
    dialog.set_accept_enabled(content.accept_enabled);
    dialog.set_test_status(content.test_status.into());
    dialog.set_test_status_is_error(content.test_status_is_error);
    dialog.set_bulk_source_index(content.bulk_source_index);
    dialog.set_bulk_replace(content.bulk_replace);
    dialog.set_about_heading(content.about_heading.into());
    dialog.set_about_credits(content.about_credits.into());
    dialog.set_about_commit(content.about_commit.into());
    dialog.set_about_built(content.about_built.into());
    dialog.set_about_run(content.about_run.into());
    dialog.set_about_notices_expanded(app.about_notices_expanded());
    dialog.set_about_copied(app.about_copied());
    set_about_notices(&dialog, app.about_notices_expanded());
    render_game_picker(&dialog, app, picker_rows);
}

/// The ID dialogs' list of the console's games: the matches of the filter, or a note while they are listed.
fn render_game_picker(dialog: &DialogModel<'_>, app: &App, rows: &mut PickerRows) {
    let filter = app
        .modal()
        .and_then(Modal::game_picker)
        .map(|picker| picker.filter.as_str())
        .unwrap_or_default();
    dialog.set_picker_filter(filter.into());
    let content = app.game_picker_content();
    dialog.set_picker_shown(content.is_some());
    let (games, highlighted, note) = match content {
        None => (Vec::new(), None, ""),
        Some(PickerContent::Loading) => (Vec::new(), None, REFRESHING_TEXT),
        Some(PickerContent::Games {
            matches,
            highlighted,
        }) => (picker_games(&matches, highlighted), highlighted, ""),
    };
    rows.show(games, Clone::clone);
    dialog.set_picker_highlighted(highlighted.map_or(NO_ROW, row_number));
    dialog.set_picker_note(note.into());
}

/// The picker's rows, the IDs as eight hex digits as in the console's table.
fn picker_games(matches: &[&Game], highlighted: Option<usize>) -> Vec<PickerGameView> {
    matches
        .iter()
        .enumerate()
        .map(|(row, game)| PickerGameView {
            title: game.title_name.as_str().into(),
            title_id: game.ids.title_id.to_string().into(),
            database_id: game.ids.database_id.to_string().into(),
            disc: game.disc.to_string().into(),
            highlighted: highlighted == Some(row),
        })
        .collect()
}

fn row_number(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

/// The notices are unpacked and handed to the window only while they are expanded.
fn set_about_notices(dialog: &DialogModel<'_>, expanded: bool) {
    let shown = dialog.get_about_notices().row_count() > 0;
    if expanded == shown {
        return;
    }
    let lines: Vec<SharedString> = if expanded {
        notices_text().lines().map(SharedString::from).collect()
    } else {
        Vec::new()
    };
    dialog.set_about_notices(ModelRc::new(VecModel::from(lines)));
}

/// What the dialog model shows for one modal.
#[derive(Default)]
struct DialogContent {
    kind: DialogKind,
    title: String,
    text: String,
    confirm_label: String,
    danger: bool,
    choices: Vec<ChoiceView>,
    fields: Vec<SharedString>,
    accept_enabled: bool,
    test_status: String,
    test_status_is_error: bool,
    bulk_source_index: i32,
    bulk_replace: bool,
    about_heading: String,
    about_credits: String,
    about_commit: String,
    about_built: String,
    about_run: String,
}

impl DialogContent {
    fn of(modal: Option<&Modal>, build: &BuildInfo) -> DialogContent {
        let Some(modal) = modal else {
            return DialogContent::default();
        };
        let shown = DialogContent {
            accept_enabled: true,
            ..DialogContent::default()
        };
        match modal {
            Modal::Message { title, text } => DialogContent {
                kind: DialogKind::Message,
                title: title.clone(),
                text: text.clone(),
                ..shown
            },
            Modal::Choice {
                title,
                text,
                choices,
                ..
            } => {
                if choices.len() > CHOICE_BUTTONS {
                    log::error!(
                        "A choice dialog offers {} choices; it has buttons for {CHOICE_BUTTONS}",
                        choices.len()
                    );
                }
                DialogContent {
                    kind: DialogKind::Choice,
                    title: title.clone(),
                    text: text.clone(),
                    choices: choices
                        .iter()
                        .map(|(_, label)| ChoiceView {
                            label: label.into(),
                        })
                        .collect(),
                    ..shown
                }
            }
            Modal::TitleId { title, label, form } => DialogContent {
                kind: DialogKind::TitleId,
                title: title.clone(),
                text: label.clone(),
                fields: vec![form.title_id.as_str().into()],
                accept_enabled: form.is_valid(),
                ..shown
            },
            Modal::UploadTarget { title, form } => DialogContent {
                kind: DialogKind::UploadTarget,
                title: title.clone(),
                fields: vec![
                    form.title_id.as_str().into(),
                    form.database_id.as_str().into(),
                ],
                accept_enabled: form.is_valid(),
                ..shown
            },
            Modal::Confirm {
                title,
                text,
                confirm_label,
                danger,
                ..
            } => DialogContent {
                kind: DialogKind::Confirm,
                title: title.clone(),
                text: text.clone(),
                confirm_label: confirm_label.clone(),
                danger: *danger,
                ..shown
            },
            Modal::Connection { form } => DialogContent::connection(form, shown),
            Modal::Bulk { form } => DialogContent::bulk(form, shown),
            Modal::About => DialogContent::about(build, shown),
        }
    }

    fn connection(form: &ConnectionForm, shown: DialogContent) -> DialogContent {
        let (test_status, test_status_is_error) = match &form.test_status {
            Some((severity, text)) => (text.clone(), matches!(severity, Severity::Error)),
            None => (String::new(), false),
        };
        DialogContent {
            kind: DialogKind::Connection,
            title: CONNECTION_TITLE.to_owned(),
            fields: [&form.host, &form.port, &form.username, &form.password]
                .map(|text| SharedString::from(text.as_str()))
                .to_vec(),
            test_status,
            test_status_is_error,
            ..shown
        }
    }

    fn about(build: &BuildInfo, shown: DialogContent) -> DialogContent {
        let run = match &build.run_url {
            Some(url) => format!("Run {url}"),
            None => String::new(),
        };
        DialogContent {
            kind: DialogKind::About,
            title: ABOUT_TITLE.to_owned(),
            about_heading: format!("{APP_NAME} {}", build.pill_text()),
            about_credits: ABOUT_CREDITS.to_owned(),
            about_commit: format!("Commit {}", build.full_hash),
            about_built: format!("Built {} for {}", build.build_date, build.target),
            about_run: run,
            ..shown
        }
    }

    fn bulk(form: &BulkForm, shown: DialogContent) -> DialogContent {
        let source_index = BULK_SOURCES
            .iter()
            .position(|source| *source == form.source)
            .unwrap_or_default();
        DialogContent {
            kind: DialogKind::Bulk,
            title: BULK_TITLE.to_owned(),
            text: bulk_text(form.game_count),
            bulk_source_index: i32::try_from(source_index).unwrap_or_default(),
            bulk_replace: form.replace_existing,
            ..shown
        }
    }
}

fn bulk_text(game_count: usize) -> String {
    if game_count == 1 {
        "Uploads a random cover from the source for the 1 game in the list if it has no cover on the console. Covers are not resized.".to_owned()
    } else {
        format!(
            "Uploads a random cover from the source for each of the {game_count} games in the list that has no cover on the console. Covers are not resized."
        )
    }
}

/// The choice behind the dialog's button `index`.
fn offered_choice(modal: Option<&Modal>, index: i32) -> Option<ChoiceId> {
    let choice = match modal {
        Some(Modal::Choice { choices, .. }) => usize::try_from(index)
            .ok()
            .and_then(|index| choices.get(index))
            .map(|(choice, _)| *choice),
        _ => None,
    };
    if choice.is_none() {
        log::error!("The window chose button {index}, which the dialog on screen does not offer");
    }
    choice
}

/// The form field behind the dialog's box `index`.
fn form_field(modal: Option<&Modal>, index: i32) -> Option<ModalField> {
    let fields: &[ModalField] = match modal {
        Some(Modal::TitleId { .. }) => &TITLE_ID_FIELDS,
        Some(Modal::UploadTarget { .. }) => &UPLOAD_TARGET_FIELDS,
        Some(Modal::Connection { .. }) => &CONNECTION_FIELDS,
        _ => &[],
    };
    let field = usize::try_from(index)
        .ok()
        .and_then(|index| fields.get(index))
        .copied();
    if field.is_none() {
        log::error!("The window edited box {index}, which the dialog on screen does not have");
    }
    field
}
