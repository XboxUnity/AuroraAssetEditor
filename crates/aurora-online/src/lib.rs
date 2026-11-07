#![forbid(unsafe_code)]

mod archive;
mod client_options;
mod cover_result;
mod endpoints;
mod loopback_resolver;
mod online_client;
mod online_error;
mod query;
mod url_encode;
mod xboxunity;

pub use client_options::ClientOptions;
pub use cover_result::CoverResult;
pub use endpoints::Endpoints;
pub use online_client::{DEFAULT_TIMEOUT, OnlineClient};
pub use online_error::OnlineError;
pub use query::{CoverQuery, CoverSource, QueryError, TitleId};
