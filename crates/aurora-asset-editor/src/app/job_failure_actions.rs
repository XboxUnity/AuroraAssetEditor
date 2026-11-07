use crate::console::PresenceCell;
use crate::jobs::{ConsoleJobKind, FailedJob, JobPanic};
use crate::online::TileImage;

use super::App;

pub const JOB_STOPPED_TEXT: &str = "stopped because of an internal error";

impl App {
    /// Releases what the job held, so the window is not left busy, then logs and shows the panic.
    pub(super) fn apply_failed_job(&mut self, job: &FailedJob, panic: &JobPanic) {
        self.release_failed_job(job);
        let context = format!("{} {JOB_STOPPED_TEXT}", job.description());
        self.report_error(&context, panic);
    }

    fn release_failed_job(&mut self, job: &FailedJob) {
        match job {
            FailedJob::Load => {
                self.active_work.loading = self.active_work.loading.saturating_sub(1)
            }
            FailedJob::Import => {
                self.active_work.importing = self.active_work.importing.saturating_sub(1);
                self.pending_import_status = None;
            }
            FailedJob::ConsoleAction(kind) => {
                self.active_work.console = None;
                if *kind == ConsoleJobKind::Bulk {
                    self.console.finish_bulk_run();
                }
            }
            FailedJob::PresenceCheck { generation, ids } => {
                if *generation == self.console.refresh_generation() {
                    self.console.table.set_presence(ids, PresenceCell::Failed);
                }
            }
            FailedJob::OnlineSearch { generation } => {
                if self.online.is_current(*generation) {
                    self.active_work.online_search = false;
                }
            }
            FailedJob::OnlineThumbnail { generation, index } => {
                self.finish_download();
                if let Some(tile) = self.current_tile_mut(*generation, *index) {
                    tile.thumbnail = TileImage::Failed;
                }
            }
            FailedJob::OnlineFull { generation, index } => {
                self.finish_download();
                if let Some(tile) = self.current_tile_mut(*generation, *index) {
                    tile.full = TileImage::Failed;
                }
            }
        }
    }
}
