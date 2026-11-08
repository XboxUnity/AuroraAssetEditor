use std::time::Duration;

use aurora_asset_editor::jobs::{Job, JobRunner};
use aurora_asset_editor::view::{MainWindowView, SPACE_SETTLE_TIME};
use i_slint_backend_testing::mock_elapsed_time;

use crate::QueuedRunner;

/// The jobs a view submits to resize its pictures, kept until the test runs them on its own thread as the
/// preview worker would.
#[derive(Clone, Default)]
pub struct PreviewJobs {
    queue: QueuedRunner,
}

impl PreviewJobs {
    pub fn new() -> PreviewJobs {
        PreviewJobs::default()
    }

    /// The runner to build the view with.
    pub fn runner(&self) -> Box<dyn JobRunner> {
        Box::new(self.queue.clone())
    }

    pub fn queued(&self) -> usize {
        self.queue.queued()
    }

    /// Runs every queued job and hands each resized picture to `view`; returns how many ran. The view shows
    /// them from its next render.
    pub fn run(&self, view: &MainWindowView) -> usize {
        let mut ran = 0;
        while let Some(job) = self.queue.next_job() {
            let Job::Preview(preview) = job else {
                panic!("a view submits only preview jobs, not {job:?}");
            };
            view.show_resized(preview.run());
            ran += 1;
        }
        ran
    }

    /// Hands every queued job back to `view` as failed, as `main` does with a preview job that panicked;
    /// returns how many failed.
    pub fn fail(&self, view: &MainWindowView) -> usize {
        let mut failed = 0;
        while let Some(job) = self.queue.next_job() {
            let Job::Preview(preview) = job else {
                panic!("a view submits only preview jobs, not {job:?}");
            };
            view.preview_failed(preview.picture, preview.area);
            failed += 1;
        }
        failed
    }
}

/// Lets the window report the space of every preview area and holds still until the view has settled the
/// spaces and asked for the copies they need. Advances the mock clock by `SPACE_SETTLE_TIME`.
pub fn settle_preview_spaces() {
    // Runs the change handlers and creates new elements first, so their reports start the settle timer now.
    mock_elapsed_time(Duration::ZERO);
    mock_elapsed_time(SPACE_SETTLE_TIME);
}
