//! The panic hook is process-wide, so this binary holds the only test that installs it.

use aurora_asset_editor::logging::{ErrorLog, install_panic_hook};
use std::panic;
use tempfile::TempDir;

#[test]
fn panics_are_recorded_with_message_and_location() {
    let dir = TempDir::new().expect("a temp dir is needed for the log file");
    let log = ErrorLog::in_dir(dir.path());
    install_panic_hook(log.clone());

    let outcome = panic::catch_unwind(|| panic!("slot ownership broken"));

    assert!(outcome.is_err());
    let text = std::fs::read_to_string(log.path()).expect("the panic creates the log");
    assert!(
        text.contains("] panic: slot ownership broken at "),
        "{text}"
    );
    assert!(text.contains("panic_hook.rs:"), "{text}");
}
