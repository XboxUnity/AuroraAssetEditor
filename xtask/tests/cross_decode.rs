#[cfg(windows)]
#[test]
fn rust_encodings_decode_identically_with_the_dll() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["xtask", "cross-decode"])
        .current_dir(workspace_root)
        .output()
        .expect("cargo xtask cross-decode starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cross-decode failed: {}\n{stdout}\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains(", 0 mismatches"),
        "unexpected summary: {stdout}"
    );
}
