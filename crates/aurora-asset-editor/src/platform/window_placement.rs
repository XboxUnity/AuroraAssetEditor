use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::window::Window as WinitWindow;
use slint::{LogicalSize, PhysicalPosition};

use crate::settings::{MonitorRect, WindowGeometry};

/// Before the window is shown: the saved size, in logical pixels.
pub fn restore_size(window: &slint::Window, geometry: Option<&WindowGeometry>) {
    let (width, height) = geometry.copied().unwrap_or_default().clamped_size();
    window.set_size(LogicalSize::new(width as f32, height as f32));
}

/// Once the window exists: the saved position, in physical pixels, when it is still on a monitor, then the
/// maximized state, so a maximized window comes back on the monitor it was on.
pub fn restore_position(window: &slint::Window, geometry: Option<&WindowGeometry>) {
    let Some(geometry) = geometry else {
        return;
    };
    let monitors = window.with_winit_window(monitor_rects).unwrap_or_default();
    if let Some((x, y)) = geometry.visible_position(&monitors) {
        window.set_position(PhysicalPosition::new(x, y));
    }
    window.set_maximized(geometry.maximized);
}

/// Where the window is now. A maximized window keeps the saved size, which is the size it returns to. When the
/// system does not report window positions (Wayland), the saved position is kept.
pub fn capture(window: &slint::Window, saved: Option<&WindowGeometry>) -> WindowGeometry {
    let saved = saved.copied().unwrap_or_default();
    let maximized = window.is_maximized();
    let (width, height) = if maximized {
        (saved.width, saved.height)
    } else {
        let size = window.size().to_logical(window.scale_factor());
        (size.width.round() as u32, size.height.round() as u32)
    };
    let (x, y) = match window.with_winit_window(WinitWindow::outer_position) {
        Some(Ok(position)) => (position.x, position.y),
        Some(Err(_not_supported)) => (saved.x, saved.y),
        None => {
            log::error!("The window position could not be read: the window has no winit window");
            (saved.x, saved.y)
        }
    };
    WindowGeometry {
        x,
        y,
        width,
        height,
        maximized,
    }
}

fn monitor_rects(window: &WinitWindow) -> Vec<MonitorRect> {
    window
        .available_monitors()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            MonitorRect {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}
