use std::env;
use std::error::Error;
use std::fmt;

pub const REFERENCES_VARIABLE: &str = "AURORA_UI_REFERENCES";
pub const CI_VARIABLE: &str = "CI";

const CHECK_VALUE: &str = "check";
const UPDATE_VALUE: &str = "update";

/// Whether a screenshot run only compares against the approved references or may rewrite them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceMode {
    Check,
    Update,
}

impl ReferenceMode {
    /// An unset or empty `references` means check. Update is refused when `ci` is set to anything
    /// non-empty, so CI can never rewrite the approved references.
    pub fn from_values(
        references: Option<&str>,
        ci: Option<&str>,
    ) -> Result<ReferenceMode, ReferenceModeError> {
        match references.unwrap_or_default() {
            "" | CHECK_VALUE => Ok(ReferenceMode::Check),
            UPDATE_VALUE if ci.is_some_and(|value| !value.is_empty()) => {
                Err(ReferenceModeError::UpdateOnCi)
            }
            UPDATE_VALUE => Ok(ReferenceMode::Update),
            other => Err(ReferenceModeError::UnknownMode(other.to_owned())),
        }
    }

    pub fn from_environment() -> Result<ReferenceMode, ReferenceModeError> {
        ReferenceMode::from_values(
            read_variable(REFERENCES_VARIABLE).as_deref(),
            read_variable(CI_VARIABLE).as_deref(),
        )
    }
}

/// A value that is not valid Unicode is read lossily: it can never equal "update", so it is
/// reported as an unknown mode, and for `CI` any value at all counts.
fn read_variable(name: &str) -> Option<String> {
    env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceModeError {
    UnknownMode(String),
    UpdateOnCi,
}

impl fmt::Display for ReferenceModeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMode(value) => write!(
                formatter,
                "{REFERENCES_VARIABLE} is \"{value}\"; use \"{CHECK_VALUE}\" or \"{UPDATE_VALUE}\""
            ),
            Self::UpdateOnCi => write!(
                formatter,
                "Reference updates are not allowed on CI; run `cargo xtask ui-screenshots update` locally and commit the result"
            ),
        }
    }
}

impl Error for ReferenceModeError {}
