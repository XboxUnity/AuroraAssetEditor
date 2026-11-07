//! The FSD asset container (FreeStyle Dash), read for import only. All fields are big-endian.

mod fsd_file;

pub use fsd_file::{FsdError, FsdFile, FsdImage};
