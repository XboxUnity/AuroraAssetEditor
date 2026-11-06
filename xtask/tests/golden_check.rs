use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

const HEADER_HEX_LENGTH: usize = 104;
const MAX_STORED_ARGB_PIXELS: u32 = 65_536;

#[derive(Deserialize)]
struct Manifest {
    fixture: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    name: String,
    width: u32,
    height: u32,
    dxt_header: String,
    argb_header: String,
    argb_video_stored: bool,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

#[test]
fn manifest_lists_every_input() {
    let codec_dir = workspace_root().join("fixtures").join("codec");
    let manifest_text =
        std::fs::read_to_string(codec_dir.join("manifest.toml")).expect("manifest.toml");
    let manifest: Manifest = toml::from_str(&manifest_text).expect("manifest parses");

    let input_names: BTreeSet<String> = std::fs::read_dir(codec_dir.join("inputs"))
        .expect("inputs directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
        .map(|path| {
            path.file_stem()
                .expect("file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let manifest_names: Vec<&str> = manifest
        .fixture
        .iter()
        .map(|fixture| fixture.name.as_str())
        .collect();
    let unique_manifest_names: BTreeSet<&str> = manifest_names.iter().copied().collect();

    assert_eq!(
        manifest_names.len(),
        unique_manifest_names.len(),
        "duplicate fixture names"
    );
    assert_eq!(
        input_names
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        unique_manifest_names
    );

    for fixture in &manifest.fixture {
        assert_eq!(
            fixture.dxt_header.len(),
            HEADER_HEX_LENGTH,
            "{} dxt_header",
            fixture.name
        );
        assert_eq!(
            fixture.argb_header.len(),
            HEADER_HEX_LENGTH,
            "{} argb_header",
            fixture.name
        );
        assert_eq!(
            fixture.argb_video_stored,
            fixture.width * fixture.height <= MAX_STORED_ARGB_PIXELS,
            "{} argb_video_stored",
            fixture.name
        );
    }
}

#[cfg(windows)]
#[test]
fn committed_corpus_matches_dll_output() {
    let status = std::process::Command::new(env!("CARGO"))
        .args(["xtask", "golden", "check"])
        .current_dir(workspace_root())
        .status()
        .expect("cargo xtask golden check starts");
    assert!(status.success(), "golden check failed: {status}");
}
