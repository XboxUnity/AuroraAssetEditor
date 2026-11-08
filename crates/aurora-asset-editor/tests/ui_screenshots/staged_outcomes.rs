use aurora_asset_editor::app::App;
use aurora_asset_editor::jobs::{
    ConsoleJobKind, ConsoleOutcome, Job, JobOutcome, OnlineJob, OnlineOutcome, Progress,
};
use aurora_console::{
    AssetPresence, AuroraRevision, ConsoleError, ConsoleStatus, Game, GameIds, GameListing,
};
use aurora_online::{CoverResult, OnlineError};
use editor_test_support::Scene;

/// The revision the staged Aurora reports.
const SAMPLE_REVISION: &str = "1655";

/// Answers the queued online search with `result` instead of running it, for an answer whose text would
/// differ from machine to machine (an error names the test server's address).
pub fn answer_search(scene: &mut Scene, result: Result<Vec<CoverResult>, OnlineError>) {
    let job = scene.take_next_job();
    let Some(Job::Online(OnlineJob::Search { generation, .. })) = job else {
        panic!("the next job is not an online search: {job:?}");
    };
    let query_text = scene
        .harness()
        .app()
        .online()
        .last_query
        .clone()
        .expect("a search that queued a job has recorded its query");
    scene.act(|app| {
        app.on_job_outcome(JobOutcome::Online(OnlineOutcome::Found {
            generation,
            query_text,
            result,
        }));
    });
}

/// Fills the table as a refresh does: Aurora answers first, which connects the link, then the listing
/// arrives. The console is never contacted.
pub fn show_games(app: &mut App, games: Vec<Game>) {
    app.refresh_games();
    app.on_job_outcome(JobOutcome::Progress(Progress::Console {
        kind: ConsoleJobKind::Refresh,
        status: ConsoleStatus::AuroraFound {
            revision: AuroraRevision::new(SAMPLE_REVISION.to_owned()),
        },
    }));
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Ok(GameListing {
            games,
            cleanup_error: None,
        }),
    }));
}

/// A refresh that fails before Aurora answers, so the link stays as it was.
pub fn refresh_fails(app: &mut App, error: ConsoleError) {
    app.refresh_games();
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Err(error),
    }));
}

/// The answer of a presence check for the game with `ids`.
pub fn mark_presence(app: &mut App, ids: &GameIds, result: Result<AssetPresence, ConsoleError>) {
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Presence {
        generation,
        ids: ids.clone(),
        result,
    }));
}

/// A bulk run reporting that it is about to handle the game at `position` of `total`.
pub fn report_bulk_progress(app: &mut App, position: usize, total: usize, title: &str) {
    app.on_job_outcome(JobOutcome::Progress(Progress::Bulk {
        position,
        total,
        title: title.to_owned(),
    }));
}
