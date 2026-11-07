use aurora_console::{ConsoleClient, ConsoleStatus, FtpSettings, Timeouts};

use super::{ConsoleJob, ConsoleOutcome, ConsoleRequest, JobOutcome, Progress, ProgressSlot};

/// Runs console jobs on one `ConsoleClient`, which stays connected between them and is replaced when a
/// request brings different settings.
pub struct ConsoleWorker {
    timeouts: Timeouts,
    running_job_progress: ProgressSlot,
    client: Option<ConsoleClient>,
}

impl ConsoleWorker {
    pub fn new(timeouts: Timeouts, running_job_progress: ProgressSlot) -> ConsoleWorker {
        ConsoleWorker {
            timeouts,
            running_job_progress,
            client: None,
        }
    }

    pub fn run(&mut self, request: ConsoleRequest) -> ConsoleOutcome {
        let client = self.client_for(request.settings);
        match request.job {
            ConsoleJob::Test => ConsoleOutcome::Tested(client.test_connection()),
            ConsoleJob::Refresh { generation } => ConsoleOutcome::Refreshed {
                generation,
                result: client.list_games(),
            },
            ConsoleJob::Presence { generation, ids } => {
                let result = client.asset_presence(&ids);
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
        ConsoleClient::new(settings, self.timeouts, move |status: &ConsoleStatus| {
            running_job_progress.send(JobOutcome::Progress(Progress::Console(status.clone())));
        })
    }
}
