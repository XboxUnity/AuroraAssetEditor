//! `cargo xtask record-online`: fetches the live cover services once and stores the responses
//! under `fixtures/online`, so the parsing tests and the recorded-HTTP server never need the internet.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use aurora_online::{Endpoints, TitleId};
use serde::{Deserialize, Serialize};

const RECORDINGS_DIRECTORY: [&str; 2] = ["fixtures", "online"];
const RECORDED_SUBDIRECTORY: &str = "recorded";
const RECORDINGS_FILE: &str = "recordings.toml";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(100);
const PAUSE_BETWEEN_REQUESTS: Duration = Duration::from_secs(3);
const THUMBNAIL_COVER_ID: &str = "225";
const KNOWN_TITLE_ID: u32 = 0x4D53_07E6;
const UNKNOWN_TITLE_ID: u32 = 0xFFFF_FFFF;
const KEYWORDS_NAME: &str = "halo_3";
const KEYWORDS_TERM: &str = "halo+3";

#[derive(Serialize)]
struct Recordings {
    response: Vec<Recorded>,
}

#[derive(Serialize)]
struct Recorded {
    name: String,
    url: String,
    path: String,
    status: u16,
    content_type: String,
    body: String,
}

struct Fetched {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

#[derive(Deserialize)]
struct CoverEntry {
    thumbnail: Option<String>,
}

struct Request {
    name: String,
    url: String,
    extension: &'static str,
    keep_body: bool,
}

struct Recorder {
    agent: ureq::Agent,
    responses: Vec<Recorded>,
    bodies: Vec<(String, Vec<u8>)>,
}

impl Recorder {
    fn new() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            responses: Vec::new(),
            bodies: Vec::new(),
        }
    }

    /// Returns the body as fetched, even when the request asks for an empty one to be stored.
    fn record(&mut self, request: Request) -> Result<Vec<u8>> {
        if !self.responses.is_empty() {
            thread::sleep(PAUSE_BETWEEN_REQUESTS);
        }
        eprintln!("GET {}", request.url);
        let fetched = fetch(&self.agent, &request.url)?;
        eprintln!("  {} ({} bytes)", fetched.status, fetched.body.len());
        let file_name = format!("{}.{}", request.name, request.extension);
        let stored = if request.keep_body {
            fetched.body.clone()
        } else {
            Vec::new()
        };
        self.responses.push(Recorded {
            name: request.name,
            path: url_path(&request.url)?,
            url: request.url,
            status: fetched.status,
            content_type: fetched.content_type,
            body: file_name.clone(),
        });
        self.bodies.push((file_name, stored));
        Ok(fetched.body)
    }

    fn write(self, directory: &Path) -> Result<()> {
        let recorded_directory = directory.join(RECORDED_SUBDIRECTORY);
        fs::create_dir_all(&recorded_directory)
            .with_context(|| format!("creating {}", recorded_directory.display()))?;
        for (file_name, body) in &self.bodies {
            let path = recorded_directory.join(file_name);
            fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
        }
        let toml = toml::to_string(&Recordings {
            response: self.responses,
        })
        .context("serialising the recordings")?;
        let recordings_path = directory.join(RECORDINGS_FILE);
        fs::write(&recordings_path, toml)
            .with_context(|| format!("writing {}", recordings_path.display()))
    }
}

pub(crate) fn record() -> Result<()> {
    let endpoints = Endpoints::live();
    let known = TitleId::new(KNOWN_TITLE_ID);
    let unknown = TitleId::new(UNKNOWN_TITLE_ID);
    let mut recorder = Recorder::new();

    let known_json = recorder.record(Request {
        name: format!("xboxunity_title_{known}"),
        url: format!("{}{known}", endpoints.xboxunity_covers),
        extension: "json",
        keep_body: true,
    })?;
    recorder.record(Request {
        name: format!("xboxunity_keywords_{KEYWORDS_NAME}"),
        url: format!("{}{KEYWORDS_TERM}", endpoints.xboxunity_covers),
        extension: "json",
        keep_body: true,
    })?;
    recorder.record(Request {
        name: format!("xboxunity_title_{unknown}"),
        url: format!("{}{unknown}", endpoints.xboxunity_covers),
        extension: "json",
        keep_body: true,
    })?;
    recorder.record(Request {
        name: format!("archive_listing_{known}"),
        url: format!("{}{known}/", endpoints.archive_covers),
        extension: "html",
        keep_body: true,
    })?;
    recorder.record(Request {
        name: format!("archive_listing_{unknown}"),
        url: format!("{}{unknown}/", endpoints.archive_covers),
        extension: "html",
        keep_body: false,
    })?;
    recorder.record(Request {
        name: format!("xboxunity_thumbnail_{THUMBNAIL_COVER_ID}"),
        url: thumbnail_url(&known_json)?,
        extension: "jpg",
        keep_body: true,
    })?;

    let count = recorder.responses.len();
    recorder.write(&recordings_directory()?)?;
    eprintln!("recorded {count} responses");
    Ok(())
}

fn fetch(agent: &ureq::Agent, url: &str) -> Result<Fetched> {
    let mut response = agent
        .get(url)
        .call()
        .with_context(|| format!("requesting {url}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .body_mut()
        .read_to_vec()
        .with_context(|| format!("reading the body of {url}"))?;
    Ok(Fetched {
        status,
        content_type,
        body,
    })
}

/// The thumbnail host is not part of `Endpoints`: the service names it in its own JSON.
fn thumbnail_url(title_json: &[u8]) -> Result<String> {
    let entries: Vec<CoverEntry> =
        serde_json::from_slice(title_json).context("parsing the recorded XboxUnity JSON")?;
    let suffix = format!("/{THUMBNAIL_COVER_ID}");
    for entry in entries {
        if let Some(thumbnail) = entry.thumbnail
            && thumbnail.ends_with(&suffix)
        {
            return Ok(thumbnail);
        }
    }
    bail!("no recorded cover has a thumbnail ending in {suffix}")
}

/// The part of `url` after the host, which is what a client puts on the request line.
fn url_path(url: &str) -> Result<String> {
    let after_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .with_context(|| format!("{url} has no scheme"))?;
    let path_start = after_scheme
        .find('/')
        .with_context(|| format!("{url} has no path"))?;
    Ok(after_scheme[path_start..].to_string())
}

fn recordings_directory() -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask has no parent directory")?;
    Ok(RECORDINGS_DIRECTORY
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part)))
}
