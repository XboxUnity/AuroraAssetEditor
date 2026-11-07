/// What the status line says once the import the user is waiting for ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingImportStatus {
    pub done: String,
    pub failed: String,
}
