use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use aurora_asset::{AssetSlot, RgbaImage, TextureFormat};
use aurora_asset_editor::app::{
    BUSY_TEXT, ENTER_KEYWORDS_TEXT, ENTER_TITLE_ID_TEXT, IMAGE_FAILED_TEXT, IMPORTING_COVER_TEXT,
    Page, SAVE_RESULT_DEFAULT_NAME, SAVE_RESULT_TITLE, SEARCH_FAILED,
};
use aurora_asset_editor::dialogs::Modal;
use aurora_asset_editor::online::{SearchMode, TileImage};
use aurora_asset_editor::status::Severity;
use aurora_console::{GameIds, HexId};
use aurora_online::{CoverQuery, CoverSource, Endpoints, OnlineClient, TitleId};
use editor_test_support::{AppHarness, DialogRequest};
use tempfile::TempDir;
use test_servers::{RecordedHttp, Route, RouteBehaviour};

const ARCHIVE_PATH: &str = "/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/";
const ONLINE_TIMEOUT: Duration = Duration::from_secs(5);
const HALO_3: &str = "4D5307E6";
const FIRST_TITLE: &str = "0000A001";
const SECOND_TITLE: &str = "0000A002";
const ERROR_TITLE: &str = "0000A500";
const COVER_WIDTH: u32 = 100;
const COVER_HEIGHT: u32 = 50;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/online")
}

fn server_with(extra: Vec<Route>) -> RecordedHttp {
    RecordedHttp::from_recordings_with_local_images(&fixtures().join("recordings.toml"), extra)
}

fn endpoints(server: &RecordedHttp) -> Endpoints {
    let base = server.base_url();
    Endpoints {
        xboxunity_covers: format!("{base}/api/Covers/"),
        archive_covers: format!("{base}{ARCHIVE_PATH}"),
    }
}

fn harness_for(server: &RecordedHttp) -> AppHarness {
    let mut harness = AppHarness::new();
    harness.with_online(endpoints(server));
    harness
}

fn png(width: u32, height: u32, shade: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([shade, (x % 251) as u8, (y % 251) as u8, 255])
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("a PNG encodes into memory");
    bytes
}

fn route(path: &str, status: u16, content_type: &str, body: Vec<u8>) -> Route {
    Route {
        path: path.to_owned(),
        status,
        content_type: content_type.to_owned(),
        body,
        behaviour: RouteBehaviour::Normal,
    }
}

fn image_route(path: &str, width: u32, height: u32, shade: u8) -> Route {
    route(path, 200, "image/png", png(width, height, shade))
}

/// One cover of an XboxUnity answer; its label is "Official cover for {name} Rating: 5".
struct Cover {
    name: &'static str,
    image: &'static str,
    thumbnail: Option<&'static str>,
}

fn cover(name: &'static str, image: &'static str, thumbnail: Option<&'static str>) -> Cover {
    Cover {
        name,
        image,
        thumbnail,
    }
}

fn label(name: &str) -> String {
    format!("Official cover for {name} Rating: 5")
}

/// The XboxUnity answer for `title_id`, with every URL on the test server.
fn listing_route(title_id: &str, covers: &[Cover]) -> Route {
    let entries: Vec<String> = covers
        .iter()
        .map(|cover| {
            let thumbnail = match cover.thumbnail {
                Some(path) => format!(r#""{{server}}{path}""#),
                None => "null".to_owned(),
            };
            format!(
                r#"{{"name":"{}","official":true,"rating":"5","url":"{{server}}{}","thumbnail":{thumbnail},"titleid":"{title_id}"}}"#,
                cover.name, cover.image
            )
        })
        .collect();
    route(
        &format!("/api/Covers/{title_id}"),
        200,
        "application/json",
        format!("[{}]", entries.join(",")).into_bytes(),
    )
}

fn search_for(harness: &mut AppHarness, title_id: &str) {
    let app = &mut *harness.app();
    app.edit_search_text(title_id);
    app.search_online();
}

/// Two covers whose thumbnails and images load.
fn two_cover_routes() -> Vec<Route> {
    vec![
        listing_route(
            FIRST_TITLE,
            &[
                cover("One", "/image/one.png", Some("/thumb/one.png")),
                cover("Two", "/image/two.png", Some("/thumb/two.png")),
            ],
        ),
        image_route("/thumb/one.png", 10, 10, 10),
        image_route("/thumb/two.png", 10, 10, 20),
        image_route("/image/one.png", COVER_WIDTH, COVER_HEIGHT, 30),
        image_route("/image/two.png", 60, 40, 40),
    ]
}

fn requests_to(server: &RecordedHttp, path: &str) -> usize {
    server
        .requests()
        .iter()
        .filter(|requested| requested.as_str() == path)
        .count()
}

fn tile_is_loaded(image: &TileImage) -> bool {
    matches!(image, TileImage::Loaded { .. })
}

#[test]
fn title_id_mode_filters_input() {
    let mut harness = AppHarness::new();
    let app = &mut *harness.app();

    app.edit_search_text("4d53-07e6zz9");
    assert_eq!(app.online().title_id_text, "4d5307e6");

    app.edit_search_text("xyz");
    assert_eq!(app.online().title_id_text, "");

    app.edit_search_text("4D5307E6G");
    assert_eq!(app.online().title_id_text, "4D5307E6");
    assert_eq!(app.online().search_text(), "4D5307E6");
}

#[test]
fn keywords_mode_keeps_free_text() {
    let mut harness = AppHarness::new();
    let app = &mut *harness.app();
    app.edit_search_text("4D5307E6");

    app.set_search_mode(SearchMode::Keywords);
    app.edit_search_text("halo 3 - zzz!");

    assert_eq!(app.online().keywords_text, "halo 3 - zzz!");
    assert_eq!(app.online().title_id_text, "4D5307E6");
    assert_eq!(app.online().search_text(), "halo 3 - zzz!");
}

#[test]
fn prefill_updates_title_id_text_only() {
    let mut harness = AppHarness::new();
    let app = &mut *harness.app();
    app.set_search_mode(SearchMode::Keywords);
    app.edit_search_text("halo 3");

    app.set_current_game(Some(aurora_asset_editor::app::CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse(HALO_3).unwrap(),
            database_id: HexId::parse("00000001").unwrap(),
        },
    }));

    assert_eq!(app.online().title_id_text, HALO_3);
    assert_eq!(app.online().keywords_text, "halo 3");
    assert_eq!(app.online().mode, SearchMode::Keywords);
}

#[test]
fn archive_source_forces_title_id_mode() {
    let mut harness = AppHarness::new();
    let app = &mut *harness.app();
    app.set_search_mode(SearchMode::Keywords);
    assert_eq!(app.online().mode, SearchMode::Keywords);

    app.set_online_source(CoverSource::InternetArchive);
    assert_eq!(app.online().mode, SearchMode::TitleId);

    app.set_search_mode(SearchMode::Keywords);
    assert_eq!(app.online().mode, SearchMode::TitleId);

    app.set_online_source(CoverSource::XboxUnity);
    app.set_search_mode(SearchMode::Keywords);
    assert_eq!(app.online().mode, SearchMode::Keywords);
}

#[test]
fn empty_title_id_is_rejected_with_message() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);

    harness.app().search_online();
    harness.run_jobs();

    assert_eq!(harness.app().status().text, ENTER_TITLE_ID_TEXT);
    assert_eq!(harness.app().status().severity, Severity::Info);
    assert!(server.requests().is_empty());
    assert_eq!(harness.queued_jobs(), 0);
    assert!(!harness.app().locks().online_search);
}

#[test]
fn blank_keywords_are_rejected_with_message() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);
    harness.app().set_search_mode(SearchMode::Keywords);
    harness.app().edit_search_text("   ");

    harness.app().search_online();
    harness.run_jobs();

    assert_eq!(harness.app().status().text, ENTER_KEYWORDS_TEXT);
    assert!(server.requests().is_empty());
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn xboxunity_search_lists_results_with_labels() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);
    search_for(&mut harness, HALO_3);

    assert_eq!(
        harness.app().status().text,
        format!("Searching XboxUnity for {HALO_3}…")
    );
    assert!(harness.app().online().results.is_empty());

    assert!(harness.run_next_job());

    let expected = OnlineClient::without_proxy(endpoints(&server), ONLINE_TIMEOUT)
        .find_covers(
            CoverSource::XboxUnity,
            &CoverQuery::TitleId(TitleId::parse(HALO_3).unwrap()),
        )
        .expect("the recorded title answers");
    let labels: Vec<String> = harness
        .app()
        .online()
        .results
        .iter()
        .map(|tile| tile.result.label.clone())
        .collect();
    let expected_labels: Vec<String> = expected.iter().map(|cover| cover.label.clone()).collect();
    assert!(!labels.is_empty());
    assert_eq!(labels, expected_labels);
    assert_eq!(
        harness.app().status().text,
        format!("{} results for {HALO_3}", expected.len())
    );
    assert_eq!(harness.app().status().severity, Severity::Info);
    assert!(!harness.app().locks().online_search);
}

#[test]
fn archive_search_shows_status_messages() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);
    harness
        .app()
        .set_online_source(CoverSource::InternetArchive);
    search_for(&mut harness, HALO_3);

    assert_eq!(
        harness.app().status().text,
        format!("Searching Internet Archive for {HALO_3}…")
    );

    assert!(harness.run_next_job());

    let expected = OnlineClient::without_proxy(endpoints(&server), ONLINE_TIMEOUT)
        .find_covers(
            CoverSource::InternetArchive,
            &CoverQuery::TitleId(TitleId::parse(HALO_3).unwrap()),
        )
        .expect("the recorded listing answers");
    assert!(!expected.is_empty());
    assert_eq!(
        harness.app().status().text,
        format!("{} results for {HALO_3}", expected.len())
    );
    assert_eq!(harness.app().online().results.len(), expected.len());
}

#[test]
fn one_result_reads_in_the_singular() {
    let server = server_with(vec![listing_route(
        FIRST_TITLE,
        &[cover("One", "/image/one.png", None)],
    )]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);

    assert!(harness.run_next_job());

    assert_eq!(
        harness.app().status().text,
        format!("1 result for {FIRST_TITLE}")
    );
}

#[test]
fn search_error_is_shown_and_logged() {
    let server = server_with(vec![route(
        &format!("/api/Covers/{ERROR_TITLE}"),
        500,
        "text/plain",
        b"broken".to_vec(),
    )]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, ERROR_TITLE);

    harness.run_jobs();

    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    assert!(
        status.text.starts_with(&format!("{SEARCH_FAILED}: ")),
        "{}",
        status.text
    );
    assert!(status.text.contains("HTTP 500"), "{}", status.text);
    let log = harness.error_log_text();
    assert!(log.contains(SEARCH_FAILED), "{log}");
    assert!(log.contains("HTTP 500"), "{log}");
    assert!(harness.app().online().results.is_empty());
    assert!(!harness.app().locks().online_search);
}

#[test]
fn a_second_search_waits_for_the_first() {
    let mut routes = two_cover_routes();
    routes.push(listing_route(
        SECOND_TITLE,
        &[cover("Other", "/image/other.png", None)],
    ));
    let server = server_with(routes);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);

    search_for(&mut harness, SECOND_TITLE);
    assert_eq!(harness.app().status().text, BUSY_TEXT);
    harness.run_jobs();

    let app = harness.app();
    let labels: Vec<&str> = app
        .online()
        .results
        .iter()
        .map(|tile| tile.result.label.as_str())
        .collect();
    assert_eq!(labels, [label("One"), label("Two")]);
    assert_eq!(app.status().text, format!("2 results for {FIRST_TITLE}"));
    assert!(!app.locks().online_search);
    drop(app);
    assert_eq!(
        requests_to(&server, &format!("/api/Covers/{SECOND_TITLE}")),
        0
    );
}

#[test]
fn thumbnails_load_lazily_and_failures_show_on_tile() {
    let server = server_with(vec![
        listing_route(
            FIRST_TITLE,
            &[
                cover("Fine", "/image/fine.png", Some("/thumb/fine.png")),
                cover("Broken", "/image/broken.png", Some("/thumb/broken.png")),
                cover("Bare", "/image/bare.png", None),
            ],
        ),
        image_route("/thumb/fine.png", 12, 8, 10),
        route("/thumb/broken.png", 404, "text/plain", b"gone".to_vec()),
        image_route("/image/bare.png", 6, 4, 20),
    ]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);

    assert!(harness.run_next_job());

    assert_eq!(harness.queued_jobs(), 3);
    assert_eq!(server.requests(), [format!("/api/Covers/{FIRST_TITLE}")]);
    assert!(
        harness
            .app()
            .online()
            .results
            .iter()
            .all(|tile| matches!(tile.thumbnail, TileImage::Pending))
    );

    harness.run_jobs();

    let app = harness.app();
    let tiles = &app.online().results;
    assert!(
        matches!(&tiles[0].thumbnail, TileImage::Loaded { pixels, .. } if pixels.dimensions() == (12, 8))
    );
    assert!(matches!(tiles[1].thumbnail, TileImage::Failed));
    assert!(
        matches!(&tiles[2].thumbnail, TileImage::Loaded { pixels, .. } if pixels.dimensions() == (6, 4))
    );
    assert_eq!(requests_to(&server, "/image/bare.png"), 1);
    drop(app);
    let log = harness.error_log_text();
    assert!(log.contains(&label("Broken")), "{log}");
    assert!(!log.contains(&label("Fine")), "{log}");
}

#[test]
fn selecting_jumps_ahead_of_waiting_thumbnails() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    assert!(harness.run_next_job());

    harness.app().select_result(1);
    assert_eq!(harness.app().status().text, "Downloading cover…".to_owned());
    assert!(harness.run_next_job());

    let app = harness.app();
    let tiles = &app.online().results;
    assert!(tile_is_loaded(&tiles[1].full));
    assert!(matches!(tiles[0].thumbnail, TileImage::Pending));
    assert!(matches!(tiles[1].thumbnail, TileImage::Pending));
    drop(app);
    assert_eq!(
        harness.app().status().text,
        format!("2 results for {FIRST_TITLE}")
    );
}

#[test]
fn selecting_a_tile_reuses_a_thumbnail_loaded_from_the_same_url() {
    let server = server_with(vec![
        listing_route(FIRST_TITLE, &[cover("Bare", "/image/bare.png", None)]),
        image_route("/image/bare.png", 6, 4, 20),
    ]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();

    harness.app().select_result(0);

    assert_eq!(harness.queued_jobs(), 0);
    assert!(tile_is_loaded(&harness.app().online().results[0].full));
    assert_eq!(requests_to(&server, "/image/bare.png"), 1);
}

#[test]
fn selecting_downloads_full_image_once() {
    let server = server_with(vec![
        listing_route(
            FIRST_TITLE,
            &[cover("Gone", "/image/gone.png", Some("/thumb/gone.png"))],
        ),
        image_route("/thumb/gone.png", 10, 10, 10),
        route("/image/gone.png", 404, "text/plain", b"gone".to_vec()),
    ]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();

    harness.app().select_result(0);
    harness.run_jobs();

    assert_eq!(requests_to(&server, "/image/gone.png"), 1);
    assert!(matches!(
        harness.app().online().results[0].full,
        TileImage::Failed
    ));
    assert_eq!(harness.app().status().text, IMAGE_FAILED_TEXT);
    assert_eq!(harness.app().status().severity, Severity::Error);
    assert!(harness.error_log_text().contains("HTTP 404"));

    harness.app().select_result(0);
    harness.run_jobs();

    assert_eq!(requests_to(&server, "/image/gone.png"), 2);
}

#[test]
fn stale_full_image_is_ignored() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();
    assert_eq!(harness.error_log_text(), "");

    harness.app().select_result(0);
    harness.app().select_result(1);
    harness.run_jobs();

    let app_guard = harness.app();
    let online = app_guard.online();
    assert!(matches!(online.results[0].full, TileImage::Pending));
    assert!(tile_is_loaded(&online.results[1].full));
    assert_eq!(online.selected, Some(1));
    let shown = online.selected_tile().expect("tile 1 is selected");
    assert_eq!(shown.result.label, label("Two"));
    assert_eq!(requests_to(&server, "/image/one.png"), 1);
    drop(app_guard);
    assert_eq!(harness.error_log_text(), "");
}

#[test]
fn use_as_cover_does_not_resize() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    harness.app().set_auto_resize(true);
    harness.app().set_use_compression(false);
    harness.app().show_page(Page::Online);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();
    harness.app().select_result(0);
    harness.run_jobs();

    harness.app().use_result_as_cover();
    assert_eq!(harness.app().status().text, IMPORTING_COVER_TEXT);
    harness.run_jobs();

    let app_guard = harness.app();
    let header = app_guard
        .pages()
        .cover
        .texture_header(AssetSlot::Cover)
        .expect("the cover holds a texture");
    assert_eq!(
        (header.width(), header.height()),
        (COVER_WIDTH, COVER_HEIGHT)
    );
    assert_eq!(header.format(), TextureFormat::LinearArgb8888);
    drop(app_guard);
    assert_eq!(harness.app().page(), Page::Online);
    assert_eq!(
        harness.app().status().text,
        "Cover set from XboxUnity result"
    );

    harness.app().set_use_compression(true);
    harness.app().use_result_as_cover();
    harness.run_jobs();

    let app_guard = harness.app();
    let header = app_guard
        .pages()
        .cover
        .texture_header(AssetSlot::Cover)
        .expect("the cover holds a texture");
    assert_eq!(
        (header.width(), header.height()),
        (COVER_WIDTH, COVER_HEIGHT)
    );
    assert_eq!(header.format(), TextureFormat::LinearDxt4);
    drop(app_guard);
    assert_eq!(harness.error_log_text(), "");
}

#[test]
fn save_result_image_reencodes_by_extension() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();
    harness.app().select_result(0);
    harness.run_jobs();
    let folder = TempDir::new().unwrap();
    let dialogs = harness.dialogs();
    dialogs.push_save(folder.path().join("saved.jpg"));
    dialogs.push_save(folder.path().join("saved.bmp"));
    dialogs.push_save(folder.path().join("saved.png"));

    harness.app().save_result_image();
    assert_eq!(harness.app().status().text, "Exported saved.jpg");
    harness.app().save_result_image();
    harness.app().save_result_image();

    let jpeg = std::fs::read(folder.path().join("saved.jpg")).unwrap();
    assert_eq!(jpeg[..2], [0xFF, 0xD8]);
    let bmp = std::fs::read(folder.path().join("saved.bmp")).unwrap();
    assert_eq!(bmp[..2], *b"BM");
    let saved: RgbaImage = image::open(folder.path().join("saved.png"))
        .expect("the PNG decodes")
        .to_rgba8();
    assert_eq!(saved.dimensions(), (COVER_WIDTH, COVER_HEIGHT));
    let requests = dialogs.requests();
    assert_eq!(requests.len(), 3);
    for request in requests {
        let DialogRequest::SaveFile(request) = request else {
            panic!("expected a save dialog, got {request:?}");
        };
        assert_eq!(request.title, SAVE_RESULT_TITLE);
        assert_eq!(request.title, "Select where to save the cover");
        assert_eq!(
            request.default_name.as_deref(),
            Some(SAVE_RESULT_DEFAULT_NAME)
        );
        assert_eq!(request.default_name.as_deref(), Some("cover.png"));
    }
}

#[test]
fn use_as_cover_names_the_source_that_was_searched() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();
    harness.app().select_result(0);
    harness.run_jobs();

    harness
        .app()
        .set_online_source(CoverSource::InternetArchive);
    harness.app().use_result_as_cover();
    harness.run_jobs();

    assert_eq!(
        harness.app().status().text,
        "Cover set from XboxUnity result"
    );
}

#[test]
fn use_as_cover_that_fails_to_encode_does_not_claim_success() {
    let wide = 9000;
    let server = server_with(vec![
        listing_route(FIRST_TITLE, &[cover("Wide", "/image/wide.png", None)]),
        image_route("/image/wide.png", wide, 1, 10),
    ]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();
    harness.app().select_result(0);
    harness.run_jobs();

    harness.app().use_result_as_cover();
    assert_eq!(harness.app().status().text, IMPORTING_COVER_TEXT);
    harness.run_jobs();

    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    assert_eq!(status.text, "Could not set the cover from XboxUnity result");
    match harness.app().modal() {
        Some(Modal::Message { title, text }) => {
            assert_eq!(title, "Could not use image");
            assert!(text.contains(&label("Wide")), "{text}");
        }
        other => panic!("expected the image error dialog, got {other:?}"),
    }
    assert!(
        harness
            .app()
            .pages()
            .cover
            .texture_header(AssetSlot::Cover)
            .is_none()
    );
    assert!(harness.error_log_text().contains("Wide"));
}

#[test]
fn selecting_zero_one_zero_applies_only_the_selected_tile() {
    let server = server_with(two_cover_routes());
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();

    harness.app().select_result(0);
    harness.app().select_result(1);
    harness.app().select_result(0);
    harness.run_jobs();

    let app_guard = harness.app();
    let online = app_guard.online();
    assert_eq!(online.selected, Some(0));
    assert!(tile_is_loaded(&online.results[0].full));
    assert!(matches!(online.results[1].full, TileImage::Pending));
    drop(app_guard);
    assert_eq!(
        harness.app().status().text,
        format!("2 results for {FIRST_TITLE}")
    );
    assert_eq!(harness.error_log_text(), "");
}

#[test]
fn undecodable_image_body_fails_the_tile_and_is_logged() {
    let server = server_with(vec![
        listing_route(FIRST_TITLE, &[cover("Junk", "/image/junk.png", None)]),
        route(
            "/image/junk.png",
            200,
            "image/png",
            b"not an image".to_vec(),
        ),
    ]);
    let mut harness = harness_for(&server);
    search_for(&mut harness, FIRST_TITLE);
    harness.run_jobs();

    assert!(matches!(
        harness.app().online().results[0].thumbnail,
        TileImage::Failed
    ));
    harness.app().select_result(0);
    harness.run_jobs();

    assert!(matches!(
        harness.app().online().results[0].full,
        TileImage::Failed
    ));
    assert_eq!(harness.app().status().text, IMAGE_FAILED_TEXT);
    let log = harness.error_log_text();
    assert!(log.contains(&label("Junk")), "{log}");
    assert!(
        log.contains("not an image format the editor reads"),
        "{log}"
    );
}

#[test]
fn recorded_results_load_their_thumbnails_from_the_local_server() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);
    search_for(&mut harness, HALO_3);

    harness.run_jobs();

    let app = harness.app();
    let tiles = &app.online().results;
    assert!(tile_is_loaded(&tiles[0].thumbnail));
    assert_eq!(requests_to(&server, "/api/boxartsm/225"), 1);
}

#[test]
fn calls_the_view_disables_do_nothing() {
    let server = server_with(Vec::new());
    let mut harness = harness_for(&server);

    harness.app().use_result_as_cover();
    harness.app().save_result_image();
    harness.app().select_result(3);
    harness
        .app()
        .set_online_source(CoverSource::InternetArchive);
    harness.app().set_search_mode(SearchMode::Keywords);

    assert_eq!(harness.queued_jobs(), 0);
    assert!(harness.app().modal().is_none());
    assert!(harness.dialogs().requests().is_empty());
    assert_eq!(harness.app().online().mode, SearchMode::TitleId);
    assert!(harness.app().online().selected.is_none());
    assert!(server.requests().is_empty());
}
