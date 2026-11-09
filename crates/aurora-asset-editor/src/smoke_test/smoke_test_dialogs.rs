use std::path::PathBuf;

use crate::platform::{DialogError, FileDialogs, OpenRequest, SaveRequest};

pub const SMOKE_TEST_NO_OTHER_DIALOGS: &str = "The smoke test opens no other dialogs";

/// Answers every save dialog with the smoke test's output file. The smoke test opens its asset without a
/// dialog, so any other dialog is a failure, which the app shows and the driver reports.
pub struct SmokeTestDialogs {
    output: PathBuf,
}

impl SmokeTestDialogs {
    pub fn new(output: PathBuf) -> SmokeTestDialogs {
        SmokeTestDialogs { output }
    }
}

fn no_other_dialogs() -> DialogError {
    DialogError {
        source: SMOKE_TEST_NO_OTHER_DIALOGS.into(),
    }
}

impl FileDialogs for SmokeTestDialogs {
    fn open_files(&mut self, _request: &OpenRequest) -> Result<Vec<PathBuf>, DialogError> {
        Err(no_other_dialogs())
    }

    fn open_file(&mut self, _request: &OpenRequest) -> Result<Option<PathBuf>, DialogError> {
        Err(no_other_dialogs())
    }

    fn save_file(&mut self, _request: &SaveRequest) -> Result<Option<PathBuf>, DialogError> {
        Ok(Some(self.output.clone()))
    }

    fn pick_folder(&mut self, _title: &str) -> Result<Option<PathBuf>, DialogError> {
        Err(no_other_dialogs())
    }
}
