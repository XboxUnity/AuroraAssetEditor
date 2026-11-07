#![forbid(unsafe_code)]

use aurora_ui::MainWindow;
use slint::{ComponentHandle, PlatformError};

fn main() -> Result<(), PlatformError> {
    let window = MainWindow::new()?;
    window.run()
}
