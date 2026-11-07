use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aurora_online::{
    ClientOptions, CoverQuery, CoverSource, DEFAULT_TIMEOUT, Endpoints, OnlineClient, OnlineError,
    TitleId,
};
use test_servers::{RecordedHttp, Route, RouteBehaviour};

const HALO_3: u32 = 0x4D5307E6;
const UNKNOWN_TITLE: u32 = 0xFFFFFFFF;
const ARCHIVE_PATH: &str = "/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/";
const TIMEOUT: Duration = Duration::from_secs(10);

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/online")
}

fn recorded(file_name: &str) -> Vec<u8> {
    fs::read(fixtures().join("recorded").join(file_name)).expect("reading a recorded body")
}

fn recorded_array_len(file_name: &str) -> usize {
    serde_json::from_slice::<serde_json::Value>(&recorded(file_name))
        .expect("the recording is JSON")
        .as_array()
        .expect("the recording is an array")
        .len()
}

fn server_with(extra: Vec<Route>) -> RecordedHttp {
    RecordedHttp::from_recordings(&fixtures().join("recordings.toml"), extra)
}

fn client_for(server: &RecordedHttp) -> OnlineClient {
    client_with_timeout(server, TIMEOUT)
}

fn endpoints_for(server: &RecordedHttp) -> Endpoints {
    let base = server.base_url();
    Endpoints {
        xboxunity_covers: format!("{base}/api/Covers/"),
        archive_covers: format!("{base}{ARCHIVE_PATH}"),
    }
}

fn client_with_timeout(server: &RecordedHttp, timeout: Duration) -> OnlineClient {
    OnlineClient::without_proxy(endpoints_for(server), timeout)
}

fn route(path: &str, status: u16, body: &[u8], behaviour: RouteBehaviour) -> Route {
    Route {
        path: path.to_string(),
        status,
        content_type: "application/octet-stream".to_string(),
        body: body.to_vec(),
        behaviour,
    }
}

fn ok_route(path: &str, body: &[u8]) -> Route {
    route(path, 200, body, RouteBehaviour::Normal)
}

fn title(value: u32) -> CoverQuery {
    CoverQuery::TitleId(TitleId::new(value))
}

#[test]
fn online_client_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OnlineClient>();
}

#[test]
fn live_endpoints_match_old_app() {
    let live = Endpoints::live();

    assert_eq!(live.xboxunity_covers, "http://xboxunity.net/api/Covers/");
    assert_eq!(
        live.archive_covers,
        "https://archive.org/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/"
    );
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(100));
}

#[test]
fn xboxunity_title_query_requests_x08_path() {
    let server = server_with(Vec::new());

    let covers = client_for(&server)
        .find_covers(CoverSource::XboxUnity, &title(HALO_3))
        .expect("the recorded title answers");

    assert_eq!(server.requests(), ["/api/Covers/4D5307E6"]);
    assert!(server.unmatched().is_empty());
    assert_eq!(
        covers.len(),
        recorded_array_len("xboxunity_title_4D5307E6.json")
    );
    let recorded_entries: Vec<serde_json::Value> =
        serde_json::from_slice(&recorded("xboxunity_title_4D5307E6.json")).expect("JSON array");
    let first = &recorded_entries[0];
    assert_eq!(covers[0].label, "Official cover for Halo 3 Rating: 5");
    assert_eq!(covers[0].image_url, first["url"].as_str().expect("url"));
    assert_eq!(
        covers[0].thumbnail_url.as_deref(),
        first["thumbnail"].as_str()
    );
}

#[test]
fn xboxunity_keywords_are_url_encoded_like_webutility() {
    let server = server_with(Vec::new());
    let query = CoverQuery::keywords("halo 3").expect("non-blank keywords");

    let covers = client_for(&server)
        .find_covers(CoverSource::XboxUnity, &query)
        .expect("the recorded search answers");

    assert_eq!(server.requests(), ["/api/Covers/halo+3"]);
    assert!(!covers.is_empty());
    assert!(server.unmatched().is_empty());
}

#[test]
fn xboxunity_unknown_title_is_empty() {
    let server = server_with(Vec::new());

    let covers = client_for(&server)
        .find_covers(CoverSource::XboxUnity, &title(UNKNOWN_TITLE))
        .expect("an empty array is a normal answer");

    assert!(covers.is_empty());
    assert_eq!(server.requests(), ["/api/Covers/FFFFFFFF"]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn xboxunity_http_error_is_reported() {
    let server = server_with(vec![route(
        "/api/Covers/00000001",
        500,
        b"oops",
        RouteBehaviour::Normal,
    )]);

    let result = client_for(&server).find_covers(CoverSource::XboxUnity, &title(1));

    assert!(matches!(
        result,
        Err(OnlineError::HttpStatus { status: 500, .. })
    ));
    assert_eq!(server.requests(), ["/api/Covers/00000001"]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn html_instead_of_json_is_invalid_response() {
    let server = server_with(vec![ok_route("/api/Covers/00000001", b"<html>hi</html>")]);

    let result = client_for(&server).find_covers(CoverSource::XboxUnity, &title(1));

    assert!(matches!(result, Err(OnlineError::InvalidResponse { .. })));
    assert!(server.unmatched().is_empty());
}

#[test]
fn archive_listing_lists_variants() {
    let server = server_with(Vec::new());
    let listing = String::from_utf8(recorded("archive_listing_4D5307E6.html")).expect("UTF-8");
    let variant_count = listing
        .lines()
        .filter(|line| {
            line.contains("<a href=\"")
                && !line.contains("<a href=\"../\"")
                && line.contains("/</a>")
                && line.matches("<a href=\"").count() == 1
        })
        .count();

    let covers = client_for(&server)
        .find_covers(CoverSource::InternetArchive, &title(HALO_3))
        .expect("the recorded listing answers");

    assert_eq!(covers.len(), 28);
    assert_eq!(covers.len(), variant_count);
    assert_eq!(covers[0].label, "TitleID: 4D5307E6 - Variant: 11234");
    assert_eq!(
        covers[0].image_url,
        format!(
            "{}{ARCHIVE_PATH}4D5307E6/11234/boxart.png",
            server.base_url()
        )
    );
    assert_eq!(covers[0].thumbnail_url, None);
    assert!(server.unmatched().is_empty());
}

#[test]
fn archive_unknown_title_is_empty() {
    let server = server_with(Vec::new());

    let covers = client_for(&server)
        .find_covers(CoverSource::InternetArchive, &title(UNKNOWN_TITLE))
        .expect("the archive 404 is zero results");

    assert!(covers.is_empty());
    assert_eq!(server.requests(), [format!("{ARCHIVE_PATH}FFFFFFFF/")]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn archive_rejects_keywords() {
    let server = server_with(Vec::new());
    let query = CoverQuery::keywords("halo").expect("non-blank keywords");

    let result = client_for(&server).find_covers(CoverSource::InternetArchive, &query);

    assert!(matches!(result, Err(OnlineError::KeywordsNeedXboxUnity)));
    assert!(server.requests().is_empty());
    assert!(server.unmatched().is_empty());
}

#[test]
fn download_image_returns_bytes() {
    let server = server_with(Vec::new());

    let bytes = client_for(&server)
        .download_image(&format!("{}/api/boxartsm/225", server.base_url()))
        .expect("the recorded thumbnail downloads");

    assert_eq!(bytes, recorded("xboxunity_thumbnail_225.jpg"));
    assert!(server.unmatched().is_empty());
}

#[test]
fn download_follows_redirect() {
    let server = server_with(vec![route(
        "/moved",
        302,
        b"",
        RouteBehaviour::Redirect {
            location: "{server}/api/boxartsm/225".to_string(),
        },
    )]);

    let bytes = client_for(&server)
        .download_image(&format!("{}/moved", server.base_url()))
        .expect("the redirect is followed");

    assert_eq!(bytes, recorded("xboxunity_thumbnail_225.jpg"));
    assert_eq!(server.requests(), ["/moved", "/api/boxartsm/225"]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn empty_image_is_error() {
    let server = server_with(vec![ok_route("/empty.png", b"")]);

    let result = client_for(&server).download_image(&format!("{}/empty.png", server.base_url()));

    assert!(matches!(result, Err(OnlineError::EmptyImage { .. })));
    assert!(server.unmatched().is_empty());
}

#[test]
fn truncated_body_is_error() {
    let server = server_with(vec![route(
        "/cut.png",
        200,
        b"0123456789",
        RouteBehaviour::Truncated { declared_len: 1000 },
    )]);

    let result = client_for(&server).download_image(&format!("{}/cut.png", server.base_url()));

    assert!(matches!(result, Err(OnlineError::Request { .. })));
    assert!(server.unmatched().is_empty());
}

#[test]
fn failed_download_is_not_retried() {
    let server = server_with(vec![route("/gone.png", 404, b"", RouteBehaviour::Normal)]);

    let result = client_for(&server).download_image(&format!("{}/gone.png", server.base_url()));

    assert!(matches!(
        result,
        Err(OnlineError::HttpStatus { status: 404, .. })
    ));
    assert_eq!(server.requests(), ["/gone.png"]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn unresponsive_server_times_out() {
    let server = server_with(vec![route(
        "/hang.png",
        200,
        b"",
        RouteBehaviour::NeverRespond,
    )]);
    let client = client_with_timeout(&server, Duration::from_millis(300));
    let started = Instant::now();

    let result = client.download_image(&format!("{}/hang.png", server.base_url()));

    let elapsed = started.elapsed();
    let error = result.unwrap_err();
    assert!(matches!(error, OnlineError::Request { .. }));
    let url = format!("{}/hang.png", server.base_url());
    assert_eq!(error.to_string(), format!("requesting {url} failed"));
    assert!(std::error::Error::source(&error).is_some());
    assert!(
        elapsed >= Duration::from_millis(250),
        "gave up after {elapsed:?}"
    );
    assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
    assert!(server.unmatched().is_empty());
}

#[test]
fn random_cover_downloads_seeded_choice() {
    let covers_json = br#"[
        {"name":"A","official":false,"rating":"1","url":"{server}/api/boxart/1"},
        {"name":"B","official":false,"rating":"2","url":"{server}/api/boxart/2"},
        {"name":"C","official":false,"rating":"3","url":"{server}/api/boxart/3"}
    ]"#;
    let server = server_with(vec![
        ok_route("/api/Covers/00000007", covers_json),
        ok_route("/api/boxart/1", b"image one"),
        ok_route("/api/boxart/2", b"image two"),
        ok_route("/api/boxart/3", b"image three"),
    ]);
    let bodies: [&[u8]; 3] = [b"image one", b"image two", b"image three"];
    let expected_index = fastrand::Rng::with_seed(7).usize(..3);

    let bytes = client_for(&server)
        .random_cover(
            CoverSource::XboxUnity,
            TitleId::new(7),
            &mut fastrand::Rng::with_seed(7),
        )
        .expect("the choice downloads")
        .expect("there are covers");

    assert_eq!(bytes, bodies[expected_index]);
    assert_eq!(
        server.requests(),
        [
            "/api/Covers/00000007".to_string(),
            format!("/api/boxart/{}", expected_index + 1)
        ]
    );
    assert!(server.unmatched().is_empty());
}

#[test]
fn random_cover_without_results_is_none() {
    let server = server_with(Vec::new());

    let cover = client_for(&server)
        .random_cover(
            CoverSource::XboxUnity,
            TitleId::new(UNKNOWN_TITLE),
            &mut fastrand::Rng::with_seed(1),
        )
        .expect("an empty result is not an error");

    assert_eq!(cover, None);
    assert_eq!(server.requests(), ["/api/Covers/FFFFFFFF"]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn random_cover_from_archive() {
    let listing = b"<a href=\"../\">Go to parent directory</a>\n<a href=\"V1/\">V1/</a>\n<a href=\"V2/\">V2/</a>\n";
    let server = server_with(vec![
        ok_route(&format!("{ARCHIVE_PATH}00000007/"), listing),
        ok_route(&format!("{ARCHIVE_PATH}00000007/V1/boxart.png"), b"first"),
        ok_route(&format!("{ARCHIVE_PATH}00000007/V2/boxart.png"), b"second"),
    ]);
    let expected_index = fastrand::Rng::with_seed(3).usize(..2);
    let bodies: [&[u8]; 2] = [b"first", b"second"];

    let bytes = client_for(&server)
        .random_cover(
            CoverSource::InternetArchive,
            TitleId::new(7),
            &mut fastrand::Rng::with_seed(3),
        )
        .expect("the choice downloads")
        .expect("there are variants");

    assert_eq!(bytes, bodies[expected_index]);
    assert!(server.unmatched().is_empty());
}

#[test]
fn archive_server_error_is_not_zero_results() {
    let server = server_with(vec![route(
        &format!("{ARCHIVE_PATH}00000001/"),
        500,
        b"oops",
        RouteBehaviour::Normal,
    )]);

    let result = client_for(&server).find_covers(CoverSource::InternetArchive, &title(1));

    assert!(matches!(
        result,
        Err(OnlineError::HttpStatus { status: 500, .. })
    ));
    assert!(server.unmatched().is_empty());
}

#[test]
fn body_over_the_size_limit_is_error() {
    let server = server_with(vec![ok_route("/big.png", &[7u8; 100])]);
    let client = OnlineClient::with_options(
        endpoints_for(&server),
        ClientOptions {
            timeout: TIMEOUT,
            use_proxy: false,
            max_response_bytes: 50,
            loopback_only: false,
        },
    );

    let result = client.download_image(&format!("{}/big.png", server.base_url()));

    assert!(matches!(result, Err(OnlineError::Request { .. })));
    assert!(server.unmatched().is_empty());
}

#[test]
fn redirect_loop_is_error() {
    let server = server_with(vec![route(
        "/loop",
        302,
        b"",
        RouteBehaviour::Redirect {
            location: "{server}/loop".to_string(),
        },
    )]);

    let result = client_for(&server).download_image(&format!("{}/loop", server.base_url()));

    assert!(matches!(result, Err(OnlineError::Request { .. })));
    assert!(server.unmatched().is_empty());
}

#[test]
fn random_cover_reaches_every_index() {
    let covers_json = br#"[
        {"name":"A","url":"{server}/api/boxart/1"},
        {"name":"B","url":"{server}/api/boxart/2"},
        {"name":"C","url":"{server}/api/boxart/3"}
    ]"#;
    let server = server_with(vec![
        ok_route("/api/Covers/00000007", covers_json),
        ok_route("/api/boxart/1", b"1"),
        ok_route("/api/boxart/2", b"2"),
        ok_route("/api/boxart/3", b"3"),
    ]);
    let client = client_for(&server);
    let mut reached = std::collections::BTreeSet::new();

    for seed in 0..40 {
        let bytes = client
            .random_cover(
                CoverSource::XboxUnity,
                TitleId::new(7),
                &mut fastrand::Rng::with_seed(seed),
            )
            .expect("the choice downloads")
            .expect("there are covers");
        reached.insert(bytes);
    }

    assert_eq!(reached.len(), 3);
    assert!(server.unmatched().is_empty());
}

#[test]
fn errors_can_cross_threads() {
    fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<OnlineError>();
    assert_send_sync::<aurora_online::QueryError>();
    assert_send_sync::<OnlineClient>();
}

#[test]
fn loopback_only_client_refuses_other_hosts_before_connecting() {
    let client = OnlineClient::with_options(
        Endpoints::live(),
        ClientOptions {
            timeout: TIMEOUT,
            loopback_only: true,
            ..ClientOptions::default()
        },
    );

    for url in [
        "http://assets.xboxunity.net/api/boxartsm/225",
        "http://192.0.2.1/x.png",
    ] {
        let started = Instant::now();
        let error = client
            .download_image(url)
            .expect_err("a remote host is refused");

        assert!(matches!(error, OnlineError::Request { .. }), "{error:?}");
        let mut chain = String::new();
        let mut next = std::error::Error::source(&error);
        while let Some(source) = next {
            chain.push_str(&source.to_string());
            next = source.source();
        }
        assert!(chain.contains("not a loopback address"), "{chain}");
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    assert!(
        client
            .find_covers(CoverSource::XboxUnity, &title(HALO_3))
            .is_err()
    );
}

#[test]
fn loopback_only_client_still_reaches_the_local_server() {
    let server = server_with(Vec::new());
    let client = OnlineClient::with_options(
        endpoints_for(&server),
        ClientOptions {
            timeout: TIMEOUT,
            loopback_only: true,
            ..ClientOptions::default()
        },
    );

    let covers = client
        .find_covers(CoverSource::XboxUnity, &title(HALO_3))
        .expect("the local server answers");

    assert!(!covers.is_empty());
}
