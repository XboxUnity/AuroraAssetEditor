/// The revision Aurora reports in answer to `SITE REVISION`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuroraRevision(String);

impl AuroraRevision {
    pub(crate) fn new(revision: String) -> AuroraRevision {
        AuroraRevision(revision)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
