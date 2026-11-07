use serde::{Deserialize, Serialize};

use crate::crafted::Crafted;
use crate::dds_sample::DdsSample;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(super) struct Manifest {
    pub(super) fixture: Vec<Fixture>,
    #[serde(default)]
    pub(super) crafted: Vec<Crafted>,
    #[serde(default)]
    pub(super) dds: Vec<DdsSample>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(super) struct Fixture {
    pub(super) name: String,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) dxt_header: String,
    pub(super) dxt_video_sha256: String,
    pub(super) dxt_decoded_sha256: String,
    pub(super) argb_header: String,
    pub(super) argb_video_sha256: String,
    pub(super) argb_decoded_sha256: String,
    pub(super) argb_video_stored: bool,
    pub(super) dxt_decoded_stored: bool,
    /// Whether the DLL decodes the video cut to the bytes it reads to the same pixels.
    pub(super) dxt_decodes_at_minimum_len: bool,
    pub(super) argb_decodes_at_minimum_len: bool,
}
