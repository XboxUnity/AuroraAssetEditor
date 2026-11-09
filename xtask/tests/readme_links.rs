use std::path::PathBuf;

const README_IMAGES: [&str; 8] = [
    "docs/images/readme/cover-light.png",
    "docs/images/readme/cover-dark.png",
    "docs/images/readme/screenshots-light.png",
    "docs/images/readme/screenshots-dark.png",
    "docs/images/readme/console-light.png",
    "docs/images/readme/console-dark.png",
    "docs/images/readme/online-light.png",
    "docs/images/readme/online-dark.png",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

fn readme() -> String {
    std::fs::read_to_string(workspace_root().join("README.md")).expect("README.md is readable")
}

/// The target of every `](target)` and `src="target"` in `text`.
fn link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for (opener, closer) in [("](", ')'), ("src=\"", '"')] {
        for piece in text.split(opener).skip(1) {
            if let Some(end) = piece.find(closer) {
                targets.push(piece[..end].to_owned());
            }
        }
    }
    targets
}

fn relative_targets(text: &str) -> Vec<String> {
    link_targets(text)
        .into_iter()
        .filter(|target| !target.starts_with("http://") && !target.starts_with("https://"))
        .filter(|target| !target.starts_with('#'))
        .collect()
}

#[test]
fn every_relative_link_in_the_readme_exists() {
    let root = workspace_root();
    let targets = relative_targets(&readme());
    assert!(!targets.is_empty(), "the README has no relative links");
    for target in &targets {
        assert!(
            root.join(target).is_file(),
            "README links to {target}, which does not exist"
        );
    }
}

#[test]
fn the_readme_references_all_eight_images() {
    let targets = relative_targets(&readme());
    for image in README_IMAGES {
        assert!(
            targets.iter().any(|target| target == image),
            "README does not reference {image}"
        );
        assert!(
            workspace_root().join(image).is_file(),
            "{image} does not exist"
        );
    }
}

#[test]
fn link_targets_finds_markdown_and_html_forms() {
    let text = "[a](x/y.png) ![b](https://e.com/z) <img src=\"p.png\"> [c](#top)";
    assert_eq!(
        relative_targets(text),
        ["x/y.png", "p.png"].map(str::to_owned)
    );
}
