#![forbid(unsafe_code)]

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the client added in the next task")
)]
mod archive;
mod cover_result;
mod endpoints;
mod query;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the client added in the next task")
)]
mod url_encode;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the client added in the next task")
)]
mod xboxunity;

pub use cover_result::CoverResult;
pub use endpoints::Endpoints;
pub use query::{CoverQuery, CoverSource, QueryError, TitleId};
