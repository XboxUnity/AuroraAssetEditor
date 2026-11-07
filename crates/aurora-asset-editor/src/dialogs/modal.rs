use super::{ChoiceId, ConnectionForm, PendingChoice, TitleIdForm};

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
    /// The console connection: Test Connection, Save and Cancel.
    Connection {
        form: ConnectionForm,
    },
}

/// How the user closed a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAnswer {
    /// OK of a form, Save of the connection form; cancels a `Choice`, which has no accept button of its own.
    Accept,
    Cancel,
    Choice(ChoiceId),
}

/// A text field inside a modal's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalField {
    TitleId,
    Host,
    Port,
    Username,
    Password,
}
