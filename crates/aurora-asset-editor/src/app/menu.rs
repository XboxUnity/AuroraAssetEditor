/// The menus that open from a button: the top bar's four and the Console page's three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Open,
    Save,
    Game,
    Gear,
    Download,
    Upload,
    Remove,
}
