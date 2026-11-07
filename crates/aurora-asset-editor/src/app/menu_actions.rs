use aurora_asset::AssetKind;

use super::{App, Menu, MenuAction};

impl App {
    pub fn open_menu(&self) -> Option<Menu> {
        self.open_menu
    }

    /// Opens `menu`, or closes it when it is already open; `None` closes any open menu.
    pub fn toggle_menu(&mut self, menu: Option<Menu>) {
        if self.open_menu == menu {
            self.open_menu = None;
        } else {
            self.open_menu = menu;
        }
    }

    /// Closes the menu, then does what its item stands for.
    pub fn menu_action(&mut self, action: MenuAction) {
        self.open_menu = None;
        match action {
            MenuAction::OpenAssetFiles => self.open_files_with_dialog(),
            MenuAction::NewAssetSet => self.new_asset_set(),
            MenuAction::SaveCover => self.save_kind(AssetKind::Cover),
            MenuAction::SaveBackground => self.save_kind(AssetKind::Background),
            MenuAction::SaveIconBanner => self.save_kind(AssetKind::IconBanner),
            MenuAction::SaveScreenshots => self.save_kind(AssetKind::Screenshots),
            MenuAction::SaveAllLocally => self.save_all_locally(),
            MenuAction::SaveAllToConsole => self.save_all_to_console(),
            MenuAction::CopyTitleId => self.copy_title_id(),
            MenuAction::CopyDatabaseId => self.copy_database_id(),
            MenuAction::ClearSelection => self.clear_current_game(),
            MenuAction::ToggleAutoResize => self.set_auto_resize(!self.settings.auto_resize),
            MenuAction::ToggleUseCompression => {
                self.set_use_compression(!self.settings.use_compression)
            }
            MenuAction::Theme(theme) => self.set_theme(theme),
            MenuAction::About => {}
            MenuAction::Download(choice) => self.download(choice),
            MenuAction::Upload(choice) => self.upload_current(choice),
            MenuAction::Remove(choice) => self.remove_from_console(choice),
        }
    }
}
