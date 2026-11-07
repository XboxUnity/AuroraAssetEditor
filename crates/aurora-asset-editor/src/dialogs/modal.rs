/// A dialog the app shows over the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Message { title: String, text: String },
}

/// How the user closed a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAnswer {
    Accept,
    Cancel,
}

/// A text field inside a modal's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalField {}
