const HANDWRITTEN: &str = include_str!("../../notices/handwritten.txt");
const THIRD_PARTY: &str = include_str!("../../notices/third-party.txt");

/// The licences of everything that ships with the app: the hand-written entries (Slint, DirectXTex, Inter)
/// followed by the Rust crates' licences that `cargo xtask notices` generated.
pub fn notices_text() -> String {
    format!("{HANDWRITTEN}\n{THIRD_PARTY}")
}
