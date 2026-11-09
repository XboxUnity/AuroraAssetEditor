use std::ffi::OsString;
use std::path::PathBuf;

use aurora_asset_editor::launch_mode::{LaunchMode, SMOKE_TEST_FLAG};

fn launch_mode(arguments: &[&str]) -> LaunchMode {
    LaunchMode::from_arguments(arguments.iter().map(OsString::from).collect())
        .expect("the arguments are accepted")
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn no_arguments_open_a_window() {
    assert_eq!(launch_mode(&[]), LaunchMode::Window { paths: Vec::new() });
}

#[test]
fn version_flag_alone_prints_the_version() {
    assert_eq!(launch_mode(&["--version"]), LaunchMode::Version);
}

#[test]
fn smoke_test_takes_an_asset_and_an_output() {
    assert_eq!(
        launch_mode(&["--smoke-test", "a.asset", "b.asset"]),
        LaunchMode::SmokeTest {
            asset: PathBuf::from("a.asset"),
            output: PathBuf::from("b.asset"),
        }
    );
}

#[test]
fn smoke_test_with_missing_paths_is_refused() {
    for arguments in [
        vec!["--smoke-test"],
        vec!["--smoke-test", "a"],
        vec!["--smoke-test", "a", "b", "c"],
    ] {
        let error = LaunchMode::from_arguments(arguments.iter().map(OsString::from).collect())
            .expect_err("--smoke-test needs exactly two paths");
        let message = error.to_string();
        assert!(message.contains(SMOKE_TEST_FLAG), "{message}");
        assert!(message.contains("asset file"), "{message}");
    }
}

#[test]
fn other_arguments_are_files() {
    assert_eq!(
        launch_mode(&["x.asset", "--version"]),
        LaunchMode::Window {
            paths: paths(&["x.asset", "--version"])
        }
    );
    assert_eq!(
        launch_mode(&["--version", "x.asset"]),
        LaunchMode::Window {
            paths: paths(&["--version", "x.asset"])
        }
    );
    assert_eq!(
        launch_mode(&["x.asset", "--smoke-test", "a", "b"]),
        LaunchMode::Window {
            paths: paths(&["x.asset", "--smoke-test", "a", "b"])
        }
    );
}
