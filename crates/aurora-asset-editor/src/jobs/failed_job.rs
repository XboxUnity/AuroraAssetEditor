use aurora_console::GameIds;
use aurora_ui::PreviewArea;

use crate::view::PictureId;

use super::{ConsoleJob, ConsoleJobKind, Job, OnlineJob};

/// A job that panicked, with what the app needs to release what the job held.
#[derive(Debug, Clone, PartialEq)]
pub enum FailedJob {
    Load,
    Import,
    /// Any console job except a presence check, which holds no lock.
    ConsoleAction(ConsoleJobKind),
    PresenceCheck {
        generation: u64,
        ids: GameIds,
    },
    OnlineSearch {
        generation: u64,
    },
    OnlineThumbnail {
        generation: u64,
        index: usize,
    },
    OnlineFull {
        generation: u64,
        index: usize,
    },
    /// Holds nothing of the app's. The view keeps showing the picture as it had it, and asks again once.
    Preview {
        picture: PictureId,
        area: PreviewArea,
    },
}

impl FailedJob {
    pub fn of(job: &Job) -> FailedJob {
        match job {
            Job::Load(_) => FailedJob::Load,
            Job::Import(_) => FailedJob::Import,
            Job::Console(request) => match &request.job {
                ConsoleJob::Presence {
                    generation, ids, ..
                } => FailedJob::PresenceCheck {
                    generation: *generation,
                    ids: ids.clone(),
                },
                other => FailedJob::ConsoleAction(other.kind()),
            },
            Job::Online(OnlineJob::Search { generation, .. }) => FailedJob::OnlineSearch {
                generation: *generation,
            },
            Job::Online(OnlineJob::Thumbnail {
                generation, index, ..
            }) => FailedJob::OnlineThumbnail {
                generation: *generation,
                index: *index,
            },
            Job::Online(OnlineJob::Full {
                generation, index, ..
            }) => FailedJob::OnlineFull {
                generation: *generation,
                index: *index,
            },
            Job::Preview(preview) => FailedJob::Preview {
                picture: preview.picture,
                area: preview.area,
            },
        }
    }

    /// What the job was doing, as the start of a sentence.
    pub fn description(&self) -> &'static str {
        match self {
            FailedJob::Load => "Loading files",
            FailedJob::Import => "Importing images",
            FailedJob::ConsoleAction(kind) => console_action_description(*kind),
            FailedJob::PresenceCheck { .. } => "Checking a game's assets on the console",
            FailedJob::OnlineSearch { .. } => "The online search",
            FailedJob::OnlineThumbnail { .. } => "Loading a result's thumbnail",
            FailedJob::OnlineFull { .. } => "Loading a result's image",
            FailedJob::Preview { .. } => "Resizing a preview",
        }
    }
}

fn console_action_description(kind: ConsoleJobKind) -> &'static str {
    match kind {
        ConsoleJobKind::Test => "Testing the connection",
        ConsoleJobKind::Refresh => "Refreshing the game list",
        ConsoleJobKind::Presence => "Checking a game's assets on the console",
        ConsoleJobKind::Download => "Downloading assets from the console",
        ConsoleJobKind::Upload => "Uploading assets to the console",
        ConsoleJobKind::Remove => "Removing assets from the console",
        ConsoleJobKind::Bulk => "Bulk covers",
    }
}
