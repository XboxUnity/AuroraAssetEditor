use aurora_console::AssetPresence;

/// One game's "Assets" cell: filled in after a refresh, one game at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceCell {
    Unknown,
    Known(AssetPresence),
    Failed,
}
