use std::path::PathBuf;
use std::time::Duration;

use aurora_asset::AssetKind;

use crate::app::App;
use crate::console::page_list;
use crate::dialogs::Modal;
use crate::status::{Locks, Severity};

pub const SMOKE_POLL_INTERVAL: Duration = Duration::from_millis(100);
pub const SMOKE_TEST_TIMEOUT: Duration = Duration::from_secs(60);

pub fn smoke_timeout_text() -> String {
    format!(
        "The smoke test did not finish within {} s",
        SMOKE_TEST_TIMEOUT.as_secs()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmokeStep {
    Waiting,
    Passed,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Open,
    Load,
    Save,
}

/// Opens one asset file through the app, saves its page to the output file, and waits for both to finish. The
/// app's timer calls `step` until it stops returning `Waiting`.
pub struct SmokeTestDriver {
    asset: PathBuf,
    output: PathBuf,
    kind: AssetKind,
    stage: Stage,
}

impl SmokeTestDriver {
    pub fn new(asset: PathBuf, output: PathBuf, kind: AssetKind) -> SmokeTestDriver {
        SmokeTestDriver {
            asset,
            output,
            kind,
            stage: Stage::Open,
        }
    }

    /// Any dialog or error the app shows fails the run, an error left by startup included: it is checked
    /// before the asset is opened, so a startup problem fails every run rather than depending on timing.
    pub fn step(&mut self, app: &mut App, elapsed: Duration) -> SmokeStep {
        if let Some(modal) = app.modal() {
            return SmokeStep::Failed(modal_text(modal));
        }
        if app.status().severity == Severity::Error {
            return SmokeStep::Failed(app.status().text.clone());
        }
        if elapsed > SMOKE_TEST_TIMEOUT {
            return SmokeStep::Failed(smoke_timeout_text());
        }
        match self.stage {
            Stage::Open => {
                app.open_paths(vec![self.asset.clone()]);
                self.stage = Stage::Load;
            }
            Stage::Load => {
                if app.pages().get(self.kind).has_content() && !app.locks().save_local {
                    app.save_kind(self.kind);
                    self.stage = Stage::Save;
                }
            }
            Stage::Save => return self.saved(app),
        }
        SmokeStep::Waiting
    }

    fn saved(&self, app: &App) -> SmokeStep {
        match self.output.try_exists() {
            Ok(true) => {}
            Ok(false) => return SmokeStep::Waiting,
            Err(error) => {
                return SmokeStep::Failed(format!(
                    "Could not check for {}: {error}",
                    self.output.display()
                ));
            }
        }
        if app.locks() != Locks::default() {
            return SmokeStep::Waiting;
        }
        // The run ends without the window's close question; a page left unsaved would mean the save did not
        // count as one.
        let unsaved = app.pages().unsaved_among(&AssetKind::ALL);
        if !unsaved.is_empty() {
            return SmokeStep::Failed(format!(
                "After the save, unsaved changes remain on {}",
                page_list(&unsaved)
            ));
        }
        SmokeStep::Passed
    }
}

/// What the dialog says: its title and its text, or the title alone for the forms.
fn modal_text(modal: &Modal) -> String {
    match modal {
        Modal::Message { title, text }
        | Modal::Choice { title, text, .. }
        | Modal::Confirm { title, text, .. } => format!("{title}: {text}"),
        Modal::TitleId { title, label, .. } => format!("{title}: {label}"),
        Modal::UploadTarget { title, .. } => title.clone(),
        Modal::Connection { .. } => "The console connection dialog opened".to_owned(),
        Modal::About => "The About dialog opened".to_owned(),
        Modal::Bulk { .. } => "The bulk covers dialog opened".to_owned(),
    }
}
