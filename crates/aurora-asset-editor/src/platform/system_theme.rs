use aurora_ui::{MainWindow, ShellModel};
use slint::ComponentHandle;
use slint::language::ColorScheme;

/// Whether the OS uses a dark theme; `ThemeChoice::System` follows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemTheme {
    pub dark: bool,
}

impl SystemTheme {
    /// The OS value as the std widgets' `Palette` reports it. Read after the window is shown and before the
    /// first render, which overwrites the palette with the chosen theme.
    pub fn read_initial(window: &MainWindow) -> SystemTheme {
        let scheme = window.global::<ShellModel>().get_palette_color_scheme();
        SystemTheme {
            dark: scheme == ColorScheme::Dark,
        }
    }

    /// From winit's `WindowEvent::ThemeChanged`.
    pub fn update(&mut self, dark: bool) {
        self.dark = dark;
    }
}
