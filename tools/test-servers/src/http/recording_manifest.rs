use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::route::{Route, RouteBehaviour};

const BODY_FOLDER: &str = "recorded";

#[derive(Deserialize)]
struct RecordingManifest {
    response: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
    path: String,
    status: u16,
    content_type: String,
    body: String,
}

/// Reads `recordings.toml`; each `body` names a file in the `recorded` folder beside it.
pub(super) fn routes_from_manifest(manifest: &Path) -> Vec<Route> {
    let text = fs::read_to_string(manifest)
        .unwrap_or_else(|error| panic!("reading {}: {error}", manifest.display()));
    let parsed: RecordingManifest = toml::from_str(&text)
        .unwrap_or_else(|error| panic!("parsing {}: {error}", manifest.display()));
    let body_folder = manifest
        .parent()
        .expect("a manifest path has a folder")
        .join(BODY_FOLDER);
    parsed
        .response
        .into_iter()
        .map(|recording| {
            let body_path = body_folder.join(&recording.body);
            let body = fs::read(&body_path)
                .unwrap_or_else(|error| panic!("reading {}: {error}", body_path.display()));
            Route {
                path: recording.path,
                status: recording.status,
                content_type: recording.content_type,
                body,
                behaviour: RouteBehaviour::Normal,
            }
        })
        .collect()
}
