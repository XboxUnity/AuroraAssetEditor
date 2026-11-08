use std::cell::RefCell;
use std::rc::Rc;

use aurora_asset::AssetKind;
use aurora_ui::{MainWindow, MenuAction as MenuItem, MenuId, PageId, ShellModel, ThemeOption};
use slint::ComponentHandle;
use slint::language::ColorScheme;

use crate::app::{App, CurrentGame, KindChoice, Menu, MenuAction, Page};
use crate::platform::SystemTheme;
use crate::settings::ThemeChoice;
use crate::status::Severity;

const NO_GAME_TEXT: &str = "no game selected";

/// Re-renders the whole window after an action.
pub(super) type Refresh = Rc<dyn Fn(&App)>;

/// Connects the top bar, sidebar, menus and status line to `app`.
pub(super) fn connect_shell(window: &MainWindow, app: &Rc<RefCell<App>>, refresh: &Refresh) {
    let shell = window.global::<ShellModel>();

    let act = action_runner(app, refresh);
    shell.on_show_page(move |page| act(&mut |app| app.show_page(page_of(page))));

    let act = action_runner(app, refresh);
    shell.on_toggle_menu(move |menu| act(&mut |app| app.toggle_menu(menu_of(menu))));

    let act = action_runner(app, refresh);
    shell.on_menu_action(move |item| {
        let action = menu_action_of(item);
        act(&mut |app| app.menu_action(action));
    });

    let act = action_runner(app, refresh);
    shell.on_show_about(move || act(&mut |app| app.show_about()));

    let act = action_runner(app, refresh);
    shell.on_stop_bulk(move || act(&mut |app| app.stop_bulk()));
}

/// Runs an action on the app, then renders the result.
pub(super) fn action_runner(
    app: &Rc<RefCell<App>>,
    refresh: &Refresh,
) -> impl Fn(&mut dyn FnMut(&mut App)) + use<> {
    let app = Rc::clone(app);
    let refresh = Rc::clone(refresh);
    move |action| {
        let mut app = app.borrow_mut();
        action(&mut app);
        refresh(&app);
    }
}

/// The window's arrow-key step as the app's; a step that does not fit is a bug of the window, so it
/// is logged and the key does nothing.
pub(super) fn arrow_step(step: i32) -> Option<isize> {
    let converted = isize::try_from(step).ok();
    if converted.is_none() {
        log::error!("The window moved the selection by {step}, which does not fit a step");
    }
    converted
}

pub(super) fn render_shell(window: &MainWindow, app: &App, system_theme: SystemTheme) {
    let shell = window.global::<ShellModel>();
    render_theme(&shell, app, system_theme);

    let build = app.build_info();
    shell.set_window_title(build.window_title().into());
    shell.set_build_pill(build.pill_text().into());
    shell.set_page(page_id(app.page()));
    shell.set_open_menu(menu_id(app.open_menu()));
    render_game(&shell, app.current_game());

    let screenshots = app.pages().get(AssetKind::Screenshots).screenshot_count();
    shell.set_screenshot_count(i32::try_from(screenshots).unwrap_or(i32::MAX));

    let status = app.status();
    shell.set_status_text(status.text.as_str().into());
    shell.set_status_is_error(status.severity == Severity::Error);
    shell.set_progress_text(status.progress.as_deref().unwrap_or_default().into());

    let locks = app.locks();
    shell.set_bulk_stop_available(locks.bulk_stop);
    shell.set_editor_locked(locks.editor);
    shell.set_save_locked(locks.save_local);
    let reason = app.save_to_console_disabled_reason().unwrap_or_default();
    shell.set_save_to_console_reason(reason.into());

    let settings = app.settings();
    shell.set_auto_resize(settings.auto_resize);
    shell.set_use_compression(settings.use_compression);
    shell.set_theme(theme_option(settings.theme));
}

fn render_theme(shell: &ShellModel, app: &App, system_theme: SystemTheme) {
    let dark = app.settings().theme.is_dark(system_theme.dark);
    shell.set_dark(dark);
    if dark {
        shell.set_color_scheme(ColorScheme::Dark);
    } else {
        shell.set_color_scheme(ColorScheme::Light);
    }
}

fn render_game(shell: &ShellModel, game: Option<&CurrentGame>) {
    shell.set_has_game(game.is_some());
    match game {
        Some(game) => {
            let title_id = game.ids.title_id.to_string();
            let database_id = game.ids.database_id.to_string();
            shell.set_chip_text(format!("{} · {title_id} · DB {database_id}", game.title).into());
            shell.set_game_title_id(title_id.into());
            shell.set_game_database_id(database_id.into());
        }
        None => {
            shell.set_chip_text(NO_GAME_TEXT.into());
            shell.set_game_title_id("".into());
            shell.set_game_database_id("".into());
        }
    }
}

fn page_id(page: Page) -> PageId {
    match page {
        Page::Cover => PageId::Cover,
        Page::Background => PageId::Background,
        Page::IconBanner => PageId::IconBanner,
        Page::Screenshots => PageId::Screenshots,
        Page::Online => PageId::Online,
        Page::Console => PageId::Console,
    }
}

fn page_of(page: PageId) -> Page {
    match page {
        PageId::Cover => Page::Cover,
        PageId::Background => Page::Background,
        PageId::IconBanner => Page::IconBanner,
        PageId::Screenshots => Page::Screenshots,
        PageId::Online => Page::Online,
        PageId::Console => Page::Console,
    }
}

fn menu_id(menu: Option<Menu>) -> MenuId {
    match menu {
        None => MenuId::None,
        Some(Menu::Open) => MenuId::Open,
        Some(Menu::Save) => MenuId::Save,
        Some(Menu::Game) => MenuId::Game,
        Some(Menu::Gear) => MenuId::Gear,
        Some(Menu::Download) => MenuId::Download,
        Some(Menu::Upload) => MenuId::Upload,
        Some(Menu::Remove) => MenuId::Remove,
    }
}

fn menu_of(menu: MenuId) -> Option<Menu> {
    match menu {
        MenuId::None => None,
        MenuId::Open => Some(Menu::Open),
        MenuId::Save => Some(Menu::Save),
        MenuId::Game => Some(Menu::Game),
        MenuId::Gear => Some(Menu::Gear),
        MenuId::Download => Some(Menu::Download),
        MenuId::Upload => Some(Menu::Upload),
        MenuId::Remove => Some(Menu::Remove),
    }
}

fn theme_option(theme: ThemeChoice) -> ThemeOption {
    match theme {
        ThemeChoice::System => ThemeOption::System,
        ThemeChoice::Light => ThemeOption::Light,
        ThemeChoice::Dark => ThemeOption::Dark,
    }
}

fn menu_action_of(item: MenuItem) -> MenuAction {
    let one = |kind| KindChoice::One(kind);
    match item {
        MenuItem::OpenAssetFiles => MenuAction::OpenAssetFiles,
        MenuItem::NewAssetSet => MenuAction::NewAssetSet,
        MenuItem::SaveCover => MenuAction::SaveCover,
        MenuItem::SaveBackground => MenuAction::SaveBackground,
        MenuItem::SaveIconBanner => MenuAction::SaveIconBanner,
        MenuItem::SaveScreenshots => MenuAction::SaveScreenshots,
        MenuItem::SaveAllLocally => MenuAction::SaveAllLocally,
        MenuItem::SaveAllToConsole => MenuAction::SaveAllToConsole,
        MenuItem::CopyTitleId => MenuAction::CopyTitleId,
        MenuItem::CopyDatabaseId => MenuAction::CopyDatabaseId,
        MenuItem::ClearSelection => MenuAction::ClearSelection,
        MenuItem::ToggleAutoResize => MenuAction::ToggleAutoResize,
        MenuItem::ToggleUseCompression => MenuAction::ToggleUseCompression,
        MenuItem::ThemeSystem => MenuAction::Theme(ThemeChoice::System),
        MenuItem::ThemeLight => MenuAction::Theme(ThemeChoice::Light),
        MenuItem::ThemeDark => MenuAction::Theme(ThemeChoice::Dark),
        MenuItem::About => MenuAction::About,
        MenuItem::DownloadAll => MenuAction::Download(KindChoice::All),
        MenuItem::DownloadCover => MenuAction::Download(one(AssetKind::Cover)),
        MenuItem::DownloadBackground => MenuAction::Download(one(AssetKind::Background)),
        MenuItem::DownloadIconBanner => MenuAction::Download(one(AssetKind::IconBanner)),
        MenuItem::DownloadScreenshots => MenuAction::Download(one(AssetKind::Screenshots)),
        MenuItem::UploadAll => MenuAction::Upload(KindChoice::All),
        MenuItem::UploadCover => MenuAction::Upload(one(AssetKind::Cover)),
        MenuItem::UploadBackground => MenuAction::Upload(one(AssetKind::Background)),
        MenuItem::UploadIconBanner => MenuAction::Upload(one(AssetKind::IconBanner)),
        MenuItem::UploadScreenshots => MenuAction::Upload(one(AssetKind::Screenshots)),
        MenuItem::RemoveAll => MenuAction::Remove(KindChoice::All),
        MenuItem::RemoveCover => MenuAction::Remove(one(AssetKind::Cover)),
        MenuItem::RemoveBackground => MenuAction::Remove(one(AssetKind::Background)),
        MenuItem::RemoveIconBanner => MenuAction::Remove(one(AssetKind::IconBanner)),
        MenuItem::RemoveScreenshots => MenuAction::Remove(one(AssetKind::Screenshots)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_steps_keep_their_direction() {
        assert_eq!(arrow_step(-1), Some(-1));
        assert_eq!(arrow_step(1), Some(1));
    }
}
