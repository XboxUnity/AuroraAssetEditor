/// What started a refresh of the game list, which decides how its failure is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RefreshOrigin {
    /// The Console page's Refresh: a connection failure gets the "Connection failed" dialog.
    #[default]
    ConsolePage,
    /// An ID dialog listing the games for its picker: the dialog stays usable with its ID boxes, so a failure
    /// goes to the status line and a toast, not to a dialog that would wait behind it.
    GamePicker,
}
