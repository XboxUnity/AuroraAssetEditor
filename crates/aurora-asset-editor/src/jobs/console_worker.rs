use std::sync::{Arc, Mutex, PoisonError};

use aurora_console::{ConsoleClient, ConsoleStatus, FtpSettings, Timeouts};

use crate::console::is_connection_failure;

use super::{
    ConsoleJob, ConsoleJobKind, ConsoleOutcome, ConsoleRequest, JobOutcome, Progress, ProgressSlot,
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

    pub fn run(&mut self, request: ConsoleRequest) -> ConsoleOutcome {
        *self
            .running_job_kind
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = request.job.kind();
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
