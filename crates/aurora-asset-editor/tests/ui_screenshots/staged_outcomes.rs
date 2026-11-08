use aurora_asset_editor::jobs::{Job, JobOutcome, OnlineJob, OnlineOutcome};
use aurora_online::{CoverResult, OnlineError};
use editor_test_support::Scene;

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
