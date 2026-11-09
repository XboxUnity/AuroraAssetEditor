#![cfg(windows)]

use std::process::Command;

use aurora_asset_editor::about::BuildInfo;

const READ_VERSION_INFO: &str = "$info = (Get-Item -LiteralPath $env:AURORA_EXE).VersionInfo; \
     $info.FileVersion; $info.ProductVersion; $info.ProductName; $info.LegalCopyright";

/// Keeps `build.rs`'s copy of `BuildInfo::pill_text` and the product strings in step with the app.
#[test]
fn exe_version_resource_names_this_build() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            READ_VERSION_INFO,
        ])
        .env("AURORA_EXE", env!("CARGO_BIN_EXE_aurora-asset-editor"))
        .output()
        .expect("could not run Windows PowerShell");
    assert!(
        output.status.success(),
        "PowerShell failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let [file_version, product_version, product_name, legal_copyright] = lines[..] else {
        panic!("expected four version info lines, got {lines:?}");
    };

    let build = BuildInfo::current();
    assert_eq!(file_version, build.version);
    assert_eq!(product_version, build.pill_text());
    assert_eq!(product_name, "Aurora Asset Editor");
    assert_eq!(legal_copyright, "Public domain (Unlicense)");
}
