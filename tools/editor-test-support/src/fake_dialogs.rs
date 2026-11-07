use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

use aurora_asset_editor::platform::{DialogError, FileDialogs, OpenRequest, SaveRequest};

/// A dialog the app opened, in the order it opened them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogRequest {
    OpenFiles(OpenRequest),
    OpenFile(OpenRequest),
    SaveFile(SaveRequest),
    PickFolder { title: String },
}

#[derive(Debug)]
enum DialogAnswer {
    Files(Vec<PathBuf>),
    Save(PathBuf),
    Folder(PathBuf),
    Cancel,
    Fail(String),
}

#[derive(Default)]
struct DialogScript {
    answers: VecDeque<DialogAnswer>,
    requests: Vec<DialogRequest>,
}

impl DialogScript {
    fn next_answer(&mut self, request: DialogRequest) -> DialogAnswer {
        let answer = self
            .answers
            .pop_front()
            .unwrap_or_else(|| panic!("the test queued no answer for {request:?}"));
        self.requests.push(request);
        answer
    }
}

/// Answers each dialog with the next answer the test queued, and panics when none is queued.
pub struct FakeDialogs {
    script: Rc<RefCell<DialogScript>>,
}

/// The test's side of `FakeDialogs`: queues answers and reads the requests.
#[derive(Clone)]
pub struct FakeDialogsHandle {
    script: Rc<RefCell<DialogScript>>,
}

impl FakeDialogs {
    pub fn new() -> (FakeDialogs, FakeDialogsHandle) {
        let script = Rc::new(RefCell::new(DialogScript::default()));
        (
            FakeDialogs {
                script: Rc::clone(&script),
            },
            FakeDialogsHandle { script },
        )
    }

    fn answer(&self, request: DialogRequest) -> DialogAnswer {
        self.script.borrow_mut().next_answer(request)
    }
}

impl FakeDialogsHandle {
    /// For an open dialog; `open_file` takes a list of exactly one file.
    pub fn push_files(&self, files: Vec<PathBuf>) {
        self.push(DialogAnswer::Files(files));
    }

    pub fn push_save(&self, path: PathBuf) {
        self.push(DialogAnswer::Save(path));
    }

    pub fn push_folder(&self, path: PathBuf) {
        self.push(DialogAnswer::Folder(path));
    }

    /// The user closes the next dialog without choosing.
    pub fn push_cancel(&self) {
        self.push(DialogAnswer::Cancel);
    }

    /// The next dialog cannot be shown; its `DialogError` carries `message`.
    pub fn push_failure(&self, message: &str) {
        self.push(DialogAnswer::Fail(message.to_owned()));
    }

    pub fn requests(&self) -> Vec<DialogRequest> {
        self.script.borrow().requests.clone()
    }

    fn push(&self, answer: DialogAnswer) {
        self.script.borrow_mut().answers.push_back(answer);
    }
}

fn dialog_error(message: String) -> DialogError {
    DialogError {
        source: Box::new(io::Error::other(message)),
    }
}

fn wrong_answer(answer: DialogAnswer, dialog: &str) -> ! {
    panic!("the test queued {answer:?} for {dialog}")
}

impl FileDialogs for FakeDialogs {
    fn open_files(&mut self, request: &OpenRequest) -> Result<Vec<PathBuf>, DialogError> {
        match self.answer(DialogRequest::OpenFiles(request.clone())) {
            DialogAnswer::Files(files) => Ok(files),
            DialogAnswer::Cancel => Ok(Vec::new()),
            DialogAnswer::Fail(message) => Err(dialog_error(message)),
            other => wrong_answer(other, "an open-files dialog"),
        }
    }

    fn open_file(&mut self, request: &OpenRequest) -> Result<Option<PathBuf>, DialogError> {
        match self.answer(DialogRequest::OpenFile(request.clone())) {
            DialogAnswer::Files(files) if files.len() == 1 => Ok(files.into_iter().next()),
            DialogAnswer::Cancel => Ok(None),
            DialogAnswer::Fail(message) => Err(dialog_error(message)),
            other => wrong_answer(other, "a single-file open dialog"),
        }
    }

    fn save_file(&mut self, request: &SaveRequest) -> Result<Option<PathBuf>, DialogError> {
        match self.answer(DialogRequest::SaveFile(request.clone())) {
            DialogAnswer::Save(path) => Ok(Some(path)),
            DialogAnswer::Cancel => Ok(None),
            DialogAnswer::Fail(message) => Err(dialog_error(message)),
            other => wrong_answer(other, "a save dialog"),
        }
    }

    fn pick_folder(&mut self, title: &str) -> Result<Option<PathBuf>, DialogError> {
        let request = DialogRequest::PickFolder {
            title: title.to_owned(),
        };
        match self.answer(request) {
            DialogAnswer::Folder(path) => Ok(Some(path)),
            DialogAnswer::Cancel => Ok(None),
            DialogAnswer::Fail(message) => Err(dialog_error(message)),
            other => wrong_answer(other, "a folder dialog"),
        }
    }
}
