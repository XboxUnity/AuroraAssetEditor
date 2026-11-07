/// One entry of a dialog's file-type list. `patterns` are file-name patterns such as `GC*.asset` or `*.asset`;
/// `*` matches every file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogFilter {
    pub name: String,
    pub patterns: Vec<String>,
}
