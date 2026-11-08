use aurora_asset_editor::app::{App, BUSY_TEXT, CurrentGame, PickerContent};
use aurora_asset_editor::dialogs::{Modal, ModalAnswer, ModalField};
use aurora_asset_editor::jobs::{ConsoleJob, ConsoleOutcome, ConsoleRequest, Job, JobOutcome};
use aurora_console::{ConsoleError, FtpSettings, Game, GameIds, GameListing, HexId};
use editor_test_support::AppHarness;

/// The code a console answers a wrong username or password with.
const LOGIN_REJECTED_CODE: u32 = 530;

fn game(title: &str, title_id: &str, database_id: i64) -> Game {
    Game {
        title_name: title.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(title_id).expect("a valid TitleID"),
            database_id: HexId::from_database_value(database_id),
        },
        media_id: HexId::parse("1A2B3C4D").expect("a valid MediaID"),
        disc: 1,
    }
}

fn games() -> Vec<Game> {
    vec![
        game("Halo 3", "4D5307E6", 1),
        game("Fable II", "4D530842", 2),
        game("Hexic HD", "58410A1C", 18),
    ]
}

/// A harness whose app has connection settings that no test connects to.
fn console_harness() -> AppHarness {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: "192.168.1.50".to_owned(),
        port: "21".to_owned(),
        ..FtpSettings::default()
    });
    harness
}

/// Lists `games` as a refresh would, and drops the presence checks that queues.
fn list_games(harness: &mut AppHarness, games: Vec<Game>) {
    {
        let app = &mut *harness.app();
        app.refresh_games();
        let generation = app.console().refresh_generation();
        app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
            generation,
            result: Ok(GameListing {
                games,
                cleanup_error: None,
            }),
        }));
    }
    while harness.take_next_job().is_some() {}
}

fn listed_harness() -> AppHarness {
    let mut harness = console_harness();
    list_games(&mut harness, games());
    harness
}

/// The titles the picker lists and the highlighted one's position.
fn picker_titles(app: &App) -> (Vec<String>, Option<usize>) {
    match app.game_picker_content() {
        Some(PickerContent::Games {
            matches,
            highlighted,
        }) => (
            matches.iter().map(|game| game.title_name.clone()).collect(),
            highlighted,
        ),
        other => panic!("the picker lists no games: {other:?}"),
    }
}

fn upload_target_ids(app: &App) -> (String, String) {
    match app.modal() {
        Some(Modal::UploadTarget { form, .. }) => (form.title_id.clone(), form.database_id.clone()),
        other => panic!("expected the upload target dialog, got {other:?}"),
    }
}

fn save_all_title_id(app: &App) -> String {
    match app.modal() {
        Some(Modal::TitleId { form, .. }) => form.title_id.clone(),
        other => panic!("expected the Save all prompt, got {other:?}"),
    }
}

fn is_refresh(job: Option<Job>) -> bool {
    matches!(
        job,
        Some(Job::Console(ConsoleRequest {
            job: ConsoleJob::Refresh { .. },
            ..
        }))
    )
}

#[test]
fn the_picker_filters_the_consoles_games_by_title_or_title_id() {
    let mut harness = listed_harness();
    let app = &mut *harness.app();
    app.save_all_to_console();
    assert_eq!(
        picker_titles(app).0,
        ["Halo 3", "Fable II", "Hexic HD"],
        "a blank filter lists every game"
    );

    app.edit_picker_filter("FABLE");
    assert_eq!(picker_titles(app).0, ["Fable II"], "titles ignore case");

    app.edit_picker_filter("5841");
    assert_eq!(picker_titles(app).0, ["Hexic HD"], "TitleIDs match too");

    app.edit_picker_filter("h");
    assert_eq!(picker_titles(app).0, ["Halo 3", "Hexic HD"]);

    app.edit_picker_filter("nothing like it");
    assert_eq!(picker_titles(app), (Vec::new(), None));
}

#[test]
fn picking_a_game_fills_both_ids_of_the_upload_target_dialog() {
    let mut harness = listed_harness();
    let app = &mut *harness.app();
    app.save_all_to_console();
    app.edit_picker_filter("fable");

    app.pick_game(0);

    assert_eq!(
        upload_target_ids(app),
        ("4D530842".to_owned(), "00000002".to_owned())
    );
    assert_eq!(app.current_game(), None, "picking leaves the current game");
}

#[test]
fn picking_a_game_fills_the_title_id_of_the_save_all_prompt() {
    let mut harness = listed_harness();
    let app = &mut *harness.app();
    app.set_current_game(Some(CurrentGame {
        title: "Halo 3".to_owned(),
        ids: games()[0].ids.clone(),
    }));
    app.save_all_locally();
    assert_eq!(save_all_title_id(app), "4D5307E6", "prefilled as before");

    app.edit_picker_filter("hexic");
    app.pick_game(0);

    assert_eq!(save_all_title_id(app), "58410A1C");
    assert_eq!(
        app.current_game().map(|game| game.title.as_str()),
        Some("Halo 3")
    );
}

#[test]
fn arrows_move_the_highlight_and_return_picks_it() {
    let mut harness = listed_harness();
    let app = &mut *harness.app();
    app.save_all_to_console();
    assert_eq!(
        picker_titles(app).1,
        None,
        "nothing is highlighted at first"
    );
    app.pick_highlighted_game();
    assert_eq!(upload_target_ids(app), (String::new(), String::new()));

    app.edit_picker_filter("4D53");
    assert_eq!(
        picker_titles(app),
        (vec!["Halo 3".to_owned(), "Fable II".to_owned()], Some(0))
    );
    app.move_picker_highlight(1);
    app.move_picker_highlight(1);
    assert_eq!(
        picker_titles(app).1,
        Some(1),
        "the highlight stops at the last match"
    );
    app.move_picker_highlight(-1);
    app.move_picker_highlight(1);

    app.pick_highlighted_game();

    assert_eq!(
        upload_target_ids(app),
        ("4D530842".to_owned(), "00000002".to_owned())
    );
    assert_eq!(
        picker_titles(app).1,
        None,
        "after a pick, Return is the dialog's OK again"
    );
}

#[test]
fn an_unlisted_console_is_listed_once_while_the_boxes_work() {
    let mut harness = console_harness();
    harness.app().save_all_locally();

    assert_eq!(harness.queued_jobs(), 1);
    assert_eq!(
        harness.app().game_picker_content(),
        Some(PickerContent::Loading)
    );
    harness.app().answer_modal(ModalAnswer::Cancel);
    harness.app().save_all_locally();
    assert_eq!(
        harness.queued_jobs(),
        1,
        "the refresh that runs is not repeated"
    );

    harness
        .app()
        .edit_modal_field(ModalField::TitleId, "4d5307e6");
    assert_eq!(save_all_title_id(&harness.app()), "4d5307e6");
    assert!(is_refresh(harness.take_next_job()));
}

#[test]
fn a_failed_listing_keeps_the_dialog_and_its_boxes() {
    let mut harness = console_harness();
    harness.app().save_all_to_console();
    assert!(is_refresh(harness.take_next_job()));

    {
        let app = &mut *harness.app();
        let generation = app.console().refresh_generation();
        app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
            generation,
            result: Err(ConsoleError::LoginRejected {
                code: LOGIN_REJECTED_CODE,
            }),
        }));
        assert!(matches!(app.modal(), Some(Modal::UploadTarget { .. })));
        assert_eq!(
            app.game_picker_content(),
            None,
            "only the ID boxes are left"
        );

        app.edit_modal_field(ModalField::UploadTitleId, "4D5307E6");
        app.edit_modal_field(ModalField::UploadDatabaseId, "00000001");
        app.answer_modal(ModalAnswer::Accept);
        assert!(
            matches!(app.modal(), Some(Modal::Message { .. })),
            "the connection failure waited behind the dialog"
        );
    }
    let app = harness.app();
    assert!(
        app.status().text.contains("Nothing to upload"),
        "OK went ahead: {}",
        app.status().text
    );
}

#[test]
fn ok_while_the_list_loads_keeps_the_upload_target_dialog_open() {
    let mut harness = console_harness();
    let app = &mut *harness.app();
    app.save_all_to_console();
    app.edit_modal_field(ModalField::UploadTitleId, "4D5307E6");
    app.edit_modal_field(ModalField::UploadDatabaseId, "00000001");

    app.answer_modal(ModalAnswer::Accept);

    assert!(matches!(app.modal(), Some(Modal::UploadTarget { .. })));
    assert_eq!(app.status().text, BUSY_TEXT);
}

#[test]
fn without_a_console_the_prompt_has_only_its_box() {
    let mut harness = AppHarness::new();
    harness.app().save_all_locally();

    assert_eq!(harness.queued_jobs(), 0, "nothing to list the games from");
    assert_eq!(harness.app().game_picker_content(), None);
    assert!(matches!(harness.app().modal(), Some(Modal::TitleId { .. })));
}
