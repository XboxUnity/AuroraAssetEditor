use aurora_asset_editor::app::App;
use aurora_asset_editor::jobs::{
    ConsoleJob, ConsoleJobKind, ConsoleOutcome, ConsoleRequest, Job, JobOutcome, OnlineJob,
    OnlineOutcome, Progress,
};
use aurora_console::{
    AssetPresence, AuroraRevision, BulkReport, ConsoleError, ConsoleStatus, Game, GameIds,
    GameListing,
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

/// The Aurora a staged console reports.
pub fn sample_revision() -> AuroraRevision {
    AuroraRevision::new(SAMPLE_REVISION.to_owned())
}

/// Answers the queued connection test with `result` instead of running it, so no console is contacted
/// and no test server's port shows in the dialog.
pub fn answer_connection_test(scene: &mut Scene, result: Result<AuroraRevision, ConsoleError>) {
    let job = scene.take_next_job();
    let Some(Job::Console(ConsoleRequest {
        job: ConsoleJob::Test,
        ..
    })) = job
    else {
        panic!("the next job is not a connection test: {job:?}");
    };
    scene.act(|app| app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Tested(result))));
}

/// Ends the running bulk run with `report`.
pub fn finish_bulk(app: &mut App, report: BulkReport) {
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Bulk(Ok(report))));
}

/// Fills the table as a refresh does: Aurora answers first, which connects the link, then the listing
/// arrives. The console is never contacted.
pub fn show_games(app: &mut App, games: Vec<Game>) {
    app.refresh_games();
    app.on_job_outcome(JobOutcome::Progress(Progress::Console {
        kind: ConsoleJobKind::Refresh,
        status: ConsoleStatus::AuroraFound {
            revision: sample_revision(),
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
    fail_running_refresh(app, error);
}

/// Ends the refresh that runs, one an ID dialog started, with `error` before Aurora answers.
pub fn fail_running_refresh(app: &mut App, error: ConsoleError) {
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
