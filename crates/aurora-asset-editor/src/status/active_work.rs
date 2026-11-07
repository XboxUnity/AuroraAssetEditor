/// What the console worker is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleActivity {
    Testing,
    Refreshing,
    /// Writes the downloaded assets into the editor.
    Downloading,
    Uploading,
    Removing,
    Bulk,
}

/// The jobs that are running or queued, which decide what the user may do meanwhile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveWork {
    pub loading: usize,
    pub importing: usize,
    pub console: Option<ConsoleActivity>,
    pub online_search: bool,
    pub online_downloads: usize,
}

/// Which controls are disabled. Every field is `true` when its controls are disabled, except `bulk_stop`,
/// which is `true` when Stop is available.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Locks {
    pub editor: bool,
    pub save_local: bool,
    pub console_actions: bool,
    pub console_editor_writes: bool,
    pub online_search: bool,
    pub use_result: bool,
    pub bulk_stop: bool,
}

impl ActiveWork {
    pub fn locks(&self) -> Locks {
        let editor = self.writes_editor();
        let console_actions = self.console.is_some();
        Locks {
            editor,
            save_local: editor,
            console_actions,
            console_editor_writes: console_actions || editor,
            online_search: self.online_search,
            use_result: editor,
            bulk_stop: self.console == Some(ConsoleActivity::Bulk),
        }
    }

    fn writes_editor(&self) -> bool {
        self.loading > 0 || self.importing > 0 || self.console == Some(ConsoleActivity::Downloading)
    }
}
