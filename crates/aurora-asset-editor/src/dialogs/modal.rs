use super::{
    BulkForm, ChoiceId, ConfirmedAction, ConnectionForm, PendingChoice, TitleIdForm,
    UploadTargetForm,
};

/// A dialog the app shows over the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Message {
        title: String,
        text: String,
    },
    /// A question with one button per choice, and Cancel.
    Choice {
        title: String,
        text: String,
        choices: Vec<(ChoiceId, String)>,
        pending: PendingChoice,
    },
    /// Save All's TitleID prompt: OK and Cancel; OK works once the form is valid.
    TitleId {
        title: String,
        label: String,
        form: TitleIdForm,
    },
    /// Save all to console: the game to upload to. OK and Cancel; OK works once both IDs are valid.
    UploadTarget {
        title: String,
        form: UploadTargetForm,
    },
    /// A question whose accepted answer runs `action`: a confirm button labelled `confirm_label` (drawn as
    /// dangerous when `danger`) and Cancel.
    Confirm {
        title: String,
        text: String,
        confirm_label: String,
        danger: bool,
        action: ConfirmedAction,
    },
    /// The console connection: Test Connection, Save and Cancel.
    Connection {
        form: ConnectionForm,
    },
    /// Bulk covers: Start (accept) and Cancel.
    Bulk {
        form: BulkForm,
    },
}

/// How the user closed a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAnswer {
    /// OK of a form, the confirm button of a `Confirm`, Save of the connection form, Start of the bulk form; cancels a `Choice`,
    /// which has no accept button of its own.
    Accept,
    Cancel,
    Choice(ChoiceId),
}

/// A text field inside a modal's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalField {
    TitleId,
    UploadTitleId,
    UploadDatabaseId,
    Host,
    Port,
    Username,
    Password,
}
