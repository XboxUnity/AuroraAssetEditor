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
const COVERS_PREFIX: &str = "/api/Covers/";
const ARCHIVE_PREFIX: &str = "/archive/";
pub const THUMBNAIL_SIZE: (u32, u32) = (90, 60);
const IMAGE_SIZE: (u32, u32) = (300, 200);
const THUMBNAIL_SEED_BASE: u8 = 30;
const IMAGE_SEED_BASE: u8 = 33;
pub const FIRST_RESULT: usize = 0;
const FAILED_RESULT: usize = 2;
const RESULTS_CONTAINER: &str = "OnlinePage::result-grid";
const FAILING_STATUS: u16 = 500;

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

/// One entry of XboxUnity's listing.
pub struct Cover {
    pub label: &'static str,
    /// Names the cover's two image routes.
    pub slug: &'static str,
    /// Whether its thumbnail and image can be downloaded.
    pub available: bool,
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
pub fn start_online(server: Option<&RecordedHttp>, theme: ThemeChoice) -> Scene {
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
    search_for_title_id(&scene, TITLE_ID);
    scene
}

fn stage_results(theme: ThemeChoice) -> Scene {
    let server = RecordedHttp::start(halo_3_routes());
    let mut scene = start_online(Some(&server), theme);
    scene.keep_alive(server);
    search_for_title_id(&scene, TITLE_ID);
    scene.run_jobs();
    scene
}

fn stage_no_results(theme: ThemeChoice) -> Scene {
    let server = RecordedHttp::start(vec![json_route(&listing_path(TITLE_ID), "[]".to_owned())]);
    let mut scene = start_online(Some(&server), theme);
    scene.keep_alive(server);
    search_for_title_id(&scene, TITLE_ID);
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
    search_for_title_id(&scene, TITLE_ID);
    answer_search(
        &mut scene,
        Err(OnlineError::HttpStatus {
            url: listing_url(),
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
    select_result_at(&mut scene, index);
    scene
}

/// Clicks the result tile at `index` and downloads its image.
pub fn select_result_at(scene: &mut Scene, index: usize) {
    // The tile's label is the one the app built from the listing's entry, not the entry's name.
    let label = scene.harness().app().online().results[index]
        .result
        .label
        .clone();
    scene.click_label_in(&label, RESULTS_CONTAINER);
    scene.run_jobs();
}

/// Types `title_id` and presses Search; the search job stays queued.
pub fn search_for_title_id(scene: &Scene, title_id: &str) {
    scene.act(|app| {
        app.edit_search_text(title_id);
        app.search_online();
    });
}

/// XboxUnity's listing for the game and the images it points to.
fn halo_3_routes() -> Vec<Route> {
    let mut routes = vec![json_route(
        &listing_path(TITLE_ID),
        listing_json(TITLE_ID, &COVERS),
    )];
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
                routes.push(png_route(
                    &path,
                    png_bytes(&sample_art(size.0, size.1, seed)),
                ));
            } else {
                routes.push(gone_route(&path));
            }
        }
    }
    routes
}

/// Where the test server answers the search for `title_id`.
pub fn listing_path(title_id: &str) -> String {
    format!("{COVERS_PREFIX}{title_id}")
}

/// The search's address on XboxUnity itself, as a failure names it.
fn listing_url() -> String {
    format!("{}{TITLE_ID}", Endpoints::live().xboxunity_covers)
}

/// The JSON of XboxUnity's answer listing `covers` for `title_id`; `{server}` is replaced by the
/// test server's address.
pub fn listing_json(title_id: &str, covers: &[Cover]) -> String {
    let entries: Vec<String> = covers
        .iter()
        .map(|cover| {
            format!(
                r#"{{"name":"{}","official":true,"rating":"5","url":"{{server}}{}","thumbnail":"{{server}}{}","titleid":"{title_id}"}}"#,
                cover.label,
                image_path(cover),
                thumbnail_path(cover),
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

pub fn thumbnail_path(cover: &Cover) -> String {
    format!("/thumb/{}.png", cover.slug)
}

pub fn image_path(cover: &Cover) -> String {
    format!("/image/{}.png", cover.slug)
}

pub fn json_route(path: &str, body: String) -> Route {
    route(path, 200, "application/json", body.into_bytes())
}

pub fn png_route(path: &str, png: Vec<u8>) -> Route {
    route(path, 200, "image/png", png)
}

fn gone_route(path: &str) -> Route {
    route(path, 404, "text/plain", b"gone".to_vec())
}

pub fn route(path: &str, status: u16, content_type: &str, body: Vec<u8>) -> Route {
    Route {
        path: path.to_owned(),
        status,
        content_type: content_type.to_owned(),
        body,
        behaviour: RouteBehaviour::Normal,
    }
}
