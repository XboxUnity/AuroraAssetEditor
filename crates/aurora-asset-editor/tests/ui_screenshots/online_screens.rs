use aurora_asset_editor::app::Page;
use aurora_asset_editor::online::SearchMode;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_online::{CoverSource, Endpoints, OnlineError};
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};
use test_servers::{RecordedHttp, Route, RouteBehaviour};

use crate::sample_content::{png_bytes, sample_art};
use crate::staged_outcomes::answer_search;

const TITLE_ID: &str = "4D5307E6";
const KEYWORDS: &str = "Halo";
const LISTING_PATH: &str = "/api/Covers/4D5307E6";
const COVERS_PREFIX: &str = "/api/Covers/";
const ARCHIVE_PREFIX: &str = "/archive/";
const THUMBNAIL_SIZE: (u32, u32) = (90, 60);
const IMAGE_SIZE: (u32, u32) = (300, 200);
const THUMBNAIL_SEED_BASE: u8 = 30;
const IMAGE_SEED_BASE: u8 = 33;
const FIRST_RESULT: usize = 0;
const FAILED_RESULT: usize = 2;
const RESULTS_CONTAINER: &str = "OnlinePage::result-grid";
const FAILING_STATUS: u16 = 500;
const LISTING_URL: &str = "http://xboxunity.net/api/Covers/4D5307E6";

/// The answers of the listing, in order. The last one's thumbnail and image are gone.
const COVERS: [Cover; 3] = [
    Cover {
        label: "Halo 3",
        slug: "halo-3",
        available: true,
    },
    Cover {
        label: "Halo 3 Limited Edition",
        slug: "halo-3-limited-edition",
        available: true,
    },
    Cover {
        label: "Halo 3 Classic",
        slug: "halo-3-classic",
        available: false,
    },
];

struct Cover {
    label: &'static str,
    /// Names the cover's two image routes.
    slug: &'static str,
    /// Whether its thumbnail and image can be downloaded.
    available: bool,
}

pub const ONLINE: ScreenArea = ScreenArea {
    title: "Online",
    screens: &[
        Screen {
            name: "online-empty",
            description: "The Online page as it opens: XboxUnity, TitleID mode, the hint to search XboxUnity or the Internet Archive.",
            stage: stage_empty,
        },
        Screen {
            name: "online-keywords",
            description: "Keywords mode with \"Halo\" typed in the search box.",
            stage: stage_keywords,
        },
        Screen {
            name: "online-archive",
            description: "The Internet Archive as the source: TitleID mode, the Keywords choice disabled.",
            stage: stage_archive,
        },
        Screen {
            name: "online-searching",
            description: "A search is running: the status says \"Searching XboxUnity for 4D5307E6…\" and Search is disabled.",
            stage: stage_searching,
        },
        Screen {
            name: "online-results",
            description: "Three results for 4D5307E6 with none selected; the third tile's thumbnail could not be loaded.",
            stage: stage_results,
        },
        Screen {
            name: "online-no-results",
            description: "A search that found nothing: \"0 results for 4D5307E6\" in the status line and in place of the grid.",
            stage: stage_no_results,
        },
        Screen {
            name: "online-selected",
            description: "The first result selected: its full image in the panel, \"Use as Cover\" enabled.",
            stage: stage_selected,
        },
        Screen {
            name: "online-selected-failed",
            description: "The third result selected: its image could not be loaded (\"Failed to load image\").",
            stage: stage_selected_failed,
        },
        Screen {
            name: "online-search-failed",
            description: "The source answered HTTP 500: the status line shows the search failure in the error colour, the grid the same text.",
            stage: stage_search_failed,
        },
        Screen {
            name: "online-title-id-missing",
            description: "Search pressed with an empty TitleID: the status asks for one.",
            stage: stage_title_id_missing,
        },
    ],
};

/// The Online page of an app whose online client reaches `server`, or of a fresh app when there is none.
fn start_online(server: Option<&RecordedHttp>, theme: ThemeChoice) -> Scene {
    let mut harness = AppHarness::new();
    if let Some(server) = server {
        let base = server.base_url();
        harness.with_online(Endpoints {
            xboxunity_covers: format!("{base}{COVERS_PREFIX}"),
            archive_covers: format!("{base}{ARCHIVE_PREFIX}"),
        });
    }
    let scene = Scene::start(harness, theme);
    scene.act(|app| app.show_page(Page::Online));
    scene
}

fn stage_empty(theme: ThemeChoice) -> Scene {
    start_online(None, theme)
}

fn stage_keywords(theme: ThemeChoice) -> Scene {
    let scene = start_online(None, theme);
    scene.act(|app| {
        app.set_search_mode(SearchMode::Keywords);
        app.edit_search_text(KEYWORDS);
    });
    scene
}

fn stage_archive(theme: ThemeChoice) -> Scene {
    let scene = start_online(None, theme);
    scene.act(|app| app.set_online_source(CoverSource::InternetArchive));
    scene
}

fn stage_searching(theme: ThemeChoice) -> Scene {
    let scene = start_online(None, theme);
    search_for_title_id(&scene);
    scene
}

fn stage_results(theme: ThemeChoice) -> Scene {
    let server = RecordedHttp::start(halo_3_routes());
    let mut scene = start_online(Some(&server), theme);
    scene.keep_alive(server);
    search_for_title_id(&scene);
    scene.run_jobs();
    scene
}

fn stage_no_results(theme: ThemeChoice) -> Scene {
    let server = RecordedHttp::start(vec![json_route(LISTING_PATH, "[]".to_owned())]);
    let mut scene = start_online(Some(&server), theme);
    scene.keep_alive(server);
    search_for_title_id(&scene);
    scene.run_jobs();
    scene
}

fn stage_selected(theme: ThemeChoice) -> Scene {
    select_result(theme, FIRST_RESULT)
}

fn stage_selected_failed(theme: ThemeChoice) -> Scene {
    select_result(theme, FAILED_RESULT)
}

fn stage_search_failed(theme: ThemeChoice) -> Scene {
    let mut scene = start_online(None, theme);
    search_for_title_id(&scene);
    answer_search(
        &mut scene,
        Err(OnlineError::HttpStatus {
            url: LISTING_URL.to_owned(),
            status: FAILING_STATUS,
        }),
    );
    scene
}

fn stage_title_id_missing(theme: ThemeChoice) -> Scene {
    let scene = start_online(None, theme);
    scene.act(|app| app.search_online());
    scene
}

/// The results page with the tile at `index` clicked and its image downloaded.
fn select_result(theme: ThemeChoice, index: usize) -> Scene {
    let mut scene = stage_results(theme);
    // The tile's label is the one the app built from the listing's entry, not the entry's name.
    let label = scene.harness().app().online().results[index]
        .result
        .label
        .clone();
    scene.click_label_in(&label, RESULTS_CONTAINER);
    scene.run_jobs();
    scene
}

/// Types Halo 3's TitleID and presses Search; the search job stays queued.
fn search_for_title_id(scene: &Scene) {
    scene.act(|app| {
        app.edit_search_text(TITLE_ID);
        app.search_online();
    });
}

/// XboxUnity's listing for the game and the images it points to.
fn halo_3_routes() -> Vec<Route> {
    let mut routes = vec![json_route(LISTING_PATH, listing_json())];
    for (index, cover) in COVERS.iter().enumerate() {
        let index = u8::try_from(index).expect("the sample has a handful of covers");
        let routes_of_cover = [
            (
                thumbnail_path(cover),
                THUMBNAIL_SIZE,
                THUMBNAIL_SEED_BASE + index,
            ),
            (image_path(cover), IMAGE_SIZE, IMAGE_SEED_BASE + index),
        ];
        for (path, size, seed) in routes_of_cover {
            if cover.available {
                routes.push(image_route(&path, size, seed));
            } else {
                routes.push(gone_route(&path));
            }
        }
    }
    routes
}

/// The JSON of XboxUnity's answer; `{server}` is replaced by the test server's address.
fn listing_json() -> String {
    let entries: Vec<String> = COVERS
        .iter()
        .map(|cover| {
            format!(
                r#"{{"name":"{}","official":true,"rating":"5","url":"{{server}}{}","thumbnail":"{{server}}{}","titleid":"{TITLE_ID}"}}"#,
                cover.label,
                image_path(cover),
                thumbnail_path(cover),
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn thumbnail_path(cover: &Cover) -> String {
    format!("/thumb/{}.png", cover.slug)
}

fn image_path(cover: &Cover) -> String {
    format!("/image/{}.png", cover.slug)
}

fn json_route(path: &str, body: String) -> Route {
    route(path, 200, "application/json", body.into_bytes())
}

fn image_route(path: &str, size: (u32, u32), seed: u8) -> Route {
    let image = sample_art(size.0, size.1, seed);
    route(path, 200, "image/png", png_bytes(&image))
}

fn gone_route(path: &str) -> Route {
    route(path, 404, "text/plain", b"gone".to_vec())
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
