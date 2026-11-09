use std::path::PathBuf;

use super::DialogError;

/// One entry of a dialog's file-type list. `patterns` are file-name patterns such as `GC*.asset` or
/// `*.asset`; `*` matches every file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub title: String,
    pub filters: Vec<FileFilter>,
    /// Indexes `filters` from 0.
    pub default_filter: usize,
    pub default_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    pub title: String,
    pub filters: Vec<FileFilter>,
    /// Indexes `filters` from 0.
    pub default_filter: usize,
    pub default_name: Option<String>,
}

/// The system's open, save and folder dialogs. An empty list or `None` is the user's cancel.
pub trait FileDialogs {
    fn open_files(&mut self, request: &OpenRequest) -> Result<Vec<PathBuf>, DialogError>;
    fn open_file(&mut self, request: &OpenRequest) -> Result<Option<PathBuf>, DialogError>;
    fn save_file(&mut self, request: &SaveRequest) -> Result<Option<PathBuf>, DialogError>;
    fn pick_folder(&mut self, title: &str) -> Result<Option<PathBuf>, DialogError>;
}
