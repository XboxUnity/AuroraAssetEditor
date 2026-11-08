/// Which action an upload belongs to, which decides how its success reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadPurpose {
    /// The console page's "Upload current".
    UploadCurrent,
    /// The Save menu's "Save all to console…".
    SaveAll,
}
