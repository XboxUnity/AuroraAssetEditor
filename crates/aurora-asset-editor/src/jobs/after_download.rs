/// What the app does with the pages a console download filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterDownload {
    StayOnConsole,
    /// Shows the first page that received content, in sidebar order; after a full set with nothing in it,
    /// the Cover page.
    OpenFirstLoaded,
}
