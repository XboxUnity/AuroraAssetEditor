use super::{ChoiceId, PendingChoice};

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
}

/// How the user closed a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAnswer {
    /// Cancels a `Choice`, which has no accept button of its own.
    Accept,
    Cancel,
    Choice(ChoiceId),
}

/// A text field inside a modal's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalField {}
