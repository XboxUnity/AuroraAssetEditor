use std::io::Read;

use flate2::read::GzDecoder;

const COMPRESSED_NOTICES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/notices.gz"));

/// The licences of everything that ships with the app: the hand-written entries (Slint, DirectXTex, Inter)
/// followed by the Rust crates' licences that `cargo xtask notices` generated. Unpacked on each call.
pub fn notices_text() -> String {
    let mut text = String::new();
    match GzDecoder::new(COMPRESSED_NOTICES).read_to_string(&mut text) {
        Ok(_) => text,
        Err(error) => {
            log::error!("The built-in notices could not be unpacked: {error}");
            format!("The notices could not be unpacked: {error}")
        }
    }
}
