use std::error::Error;
use std::fmt;

use super::ChoiceId;

/// A choice dialog was answered with a button it does not offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnofferedChoice(pub ChoiceId);

impl fmt::Display for UnofferedChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the dialog has no {:?} button", self.0)
    }
}

impl Error for UnofferedChoice {}
