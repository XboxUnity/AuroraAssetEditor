#![forbid(unsafe_code)]

mod archive;
mod client;
mod cover_result;
mod endpoints;
mod online_error;
mod query;
mod url_encode;
mod xboxunity;

pub use client::{DEFAULT_TIMEOUT, OnlineClient};
pub use cover_result::CoverResult;
pub use endpoints::Endpoints;
pub use online_error::OnlineError;
pub use query::{CoverQuery, CoverSource, QueryError, TitleId};
