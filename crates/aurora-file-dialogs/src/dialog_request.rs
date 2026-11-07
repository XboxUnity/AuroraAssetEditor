use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use super::DialogFilter;

/// What an open or save dialog shows.
#[derive(Debug, Clone)]
pub struct DialogRequest<'a> {
    pub title: String,
    pub filters: Vec<DialogFilter>,
    /// Indexes `filters` from 0.
    pub default_filter: usize,
    pub default_name: Option<String>,
    pub parent: Option<DialogParent<'a>>,
}

/// The window a dialog is modal to.
#[derive(Debug, Clone, Copy)]
pub struct DialogParent<'a> {
    pub window: WindowHandle<'a>,
    pub display: DisplayHandle<'a>,
}

impl HasWindowHandle for DialogParent<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(self.window)
    }
}

impl HasDisplayHandle for DialogParent<'_> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(self.display)
    }
}
