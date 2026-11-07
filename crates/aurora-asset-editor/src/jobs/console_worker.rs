use std::ops::ControlFlow;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use aurora_asset::AssetKind;
use aurora_console::{
    BulkError, BulkReport, ConsoleClient, ConsoleStatus, FtpSettings, GameIds, Timeouts,
    run_bulk_covers,
};
use aurora_online::OnlineClient;

use crate::console::{OnlineCoverProvider, is_connection_failure};
use crate::editor::decode_kind;

use super::{
    BulkRequest, ConsoleJob, ConsoleJobKind, ConsoleOutcome, ConsoleRequest, JobOutcome,
    KindDownload, Progress, ProgressSlot, TransferReport,
};

/// Runs console jobs on one `ConsoleClient`, which stays connected between them and is replaced when a
/// request brings different settings.
pub struct ConsoleWorker {
    timeouts: Timeouts,
    running_job_progress: ProgressSlot,
    /// What the running job is, which the client's status callback stamps on its reports.
    running_job_kind: Arc<Mutex<ConsoleJobKind>>,
    client: Option<ConsoleClient>,
}

impl ConsoleWorker {
    pub fn new(timeouts: Timeouts, running_job_progress: ProgressSlot) -> ConsoleWorker {
        ConsoleWorker {
            timeouts,
            running_job_progress,
            running_job_kind: Arc::new(Mutex::new(ConsoleJobKind::Test)),
            client: None,
        }
    }

    /// `online` is cloned for the jobs that fetch from the internet.
    pub fn run(&mut self, request: ConsoleRequest, online: &OnlineClient) -> ConsoleOutcome {
        *self
            .running_job_kind
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = request.job.kind();
        let progress = self.running_job_progress.clone();
        let client = self.client_for(request.settings);
        match request.job {
            ConsoleJob::Test => ConsoleOutcome::Tested(client.test_connection()),
            ConsoleJob::Refresh { generation } => ConsoleOutcome::Refreshed {
                generation,
                result: client.list_games(),
            },
            ConsoleJob::Presence {
                generation,
                ids,
                gate,
            } => {
                if gate.should_skip(generation) {
                    return ConsoleOutcome::PresenceSkipped { generation, ids };
                }
                let result = client.asset_presence(&ids);
                if result.as_ref().is_err_and(is_connection_failure) {
                    gate.mark_unreachable(generation);
                }
                ConsoleOutcome::Presence {
                    generation,
                    ids,
                    result,
                }
            }
            ConsoleJob::Download { ids, kinds, after } => {
                let results = download_kinds(client, &ids, &kinds);
                ConsoleOutcome::Downloaded {
                    ids,
                    after,
                    results,
                }
            }
            ConsoleJob::Upload { ids, uploads } => {
                let mut report = TransferReport::new(ids.clone());
                for upload in uploads {
                    let result = client.upload_asset(&ids, upload.kind, &upload.bytes);
                    if !report.record(upload.kind, result) {
                        break;
                    }
                }
                ConsoleOutcome::Uploaded(report)
            }
            ConsoleJob::Remove { ids, kinds } => {
                let mut report = TransferReport::new(ids.clone());
                for kind in kinds {
                    let result = client.remove_asset(&ids, kind);
                    if !report.record(kind, result) {
                        break;
                    }
                }
                ConsoleOutcome::Removed(report)
            }
            ConsoleJob::Bulk(bulk) => {
                ConsoleOutcome::Bulk(run_bulk(client, online, &progress, bulk))
            }
        }
    }

    fn client_for(&mut self, settings: FtpSettings) -> &mut ConsoleClient {
        let client = match self.client.take() {
            Some(client) if client.settings() == &settings => client,
            _ => self.new_client(settings),
        };
        self.client.insert(client)
    }

    fn new_client(&self, settings: FtpSettings) -> ConsoleClient {
        let running_job_progress = self.running_job_progress.clone();
        let running_job_kind = Arc::clone(&self.running_job_kind);
        ConsoleClient::new(settings, self.timeouts, move |status: &ConsoleStatus| {
            let kind = *running_job_kind
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            running_job_progress.send(JobOutcome::Progress(Progress::Console {
                kind,
                status: status.clone(),
            }));
        })
    }
}

/// The stop flag is read before each game's progress is reported, so Stop ends the run before the game
/// that is next.
fn run_bulk(
    client: &mut ConsoleClient,
    online: &OnlineClient,
    progress: &ProgressSlot,
    request: BulkRequest,
) -> Result<BulkReport, BulkError> {
    let BulkRequest {
        games,
        options,
        source,
        stop,
    } = request;
    let mut provider = OnlineCoverProvider::new(online.clone(), source, fastrand::Rng::new());
    run_bulk_covers(client, &games, &mut provider, options, |bulk_progress| {
        if stop.load(Ordering::SeqCst) {
            return ControlFlow::Break(());
        }
        progress.send(JobOutcome::Progress(Progress::Bulk {
            position: bulk_progress.position,
            total: bulk_progress.total,
            title: bulk_progress.game.title_name.clone(),
        }));
        ControlFlow::Continue(())
    })
}

/// A file without entries of its kind counts as missing, like one that is not there (U9).
fn download_kinds(
    client: &mut ConsoleClient,
    ids: &GameIds,
    kinds: &[AssetKind],
) -> Vec<KindDownload> {
    let mut downloads = Vec::new();
    for &kind in kinds {
        let result = client.download_asset(ids, kind).map(|file| {
            file.filter(|file| file.has_entries_for(kind))
                .map(|file| Box::new(decode_kind(&file, kind)))
        });
        let failed = result.is_err();
        downloads.push(KindDownload { kind, result });
        if failed {
            break;
        }
    }
    downloads
}
