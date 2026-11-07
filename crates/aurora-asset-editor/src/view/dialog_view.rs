use std::cell::RefCell;
use std::rc::Rc;

use aurora_online::CoverSource;
use aurora_ui::{ChoiceView, DialogKind, DialogModel, MainWindow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::app::App;
use crate::dialogs::{BulkForm, ChoiceId, ConnectionForm, Modal, ModalAnswer, ModalField};
use crate::status::Severity;

use super::shell_view::{Refresh, action_runner};

/// The choice dialog draws a button for each of the app's choices, at most two (Icon or Banner, Replace or
/// Add to free slots).
const CHOICE_BUTTONS: usize = 2;
const CONNECTION_TITLE: &str = "Console connection";
const BULK_TITLE: &str = "Bulk actions";
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
}

pub(super) fn render_dialogs(window: &MainWindow, app: &App) {
    let content = DialogContent::of(app.modal());
    let dialog = window.global::<DialogModel>();
    dialog.set_kind(content.kind);
    dialog.set_title(content.title.into());
    dialog.set_text(content.text.into());
    dialog.set_confirm_label(content.confirm_label.into());
    dialog.set_danger(content.danger);
    dialog.set_choices(ModelRc::new(VecModel::from(content.choices)));
    dialog.set_fields(ModelRc::new(VecModel::from(content.fields)));
    dialog.set_accept_enabled(content.accept_enabled);
    dialog.set_test_status(content.test_status.into());
    dialog.set_test_status_is_error(content.test_status_is_error);
    dialog.set_bulk_source_index(content.bulk_source_index);
    dialog.set_bulk_replace(content.bulk_replace);
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
}

impl DialogContent {
    fn of(modal: Option<&Modal>) -> DialogContent {
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
