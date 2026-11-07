use serde::{Deserialize, Serialize};

pub const MIN_WIDTH: u32 = 760;
pub const MIN_HEIGHT: u32 = 500;

const GRAB_STRIP_HEIGHT: i64 = 32;
const GRAB_STRIP_MIN_WIDTH: i64 = 64;
const DEFAULT_WIDTH: u32 = 1024;
const DEFAULT_HEIGHT: u32 = 680;

/// A maximized window's reported position lies outside its monitor by the invisible resize border (about 8 px at
/// 100 % scale on Windows). It counts as visible when its top-left corner, moved in by this much, is on a monitor.
const MAXIMIZED_BORDER_TOLERANCE: i64 = 16;

/// `x` and `y` are physical pixels (winit `outer_position`); `width` and `height` are logical pixels (Slint's window size).
/// Missing fields take the default window size, position 0,0 and not maximized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

/// A monitor's area in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        WindowGeometry {
            x: 0,
            y: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            maximized: false,
        }
    }
}

impl WindowGeometry {
    pub fn clamped_size(&self) -> (u32, u32) {
        (self.width.max(MIN_WIDTH), self.height.max(MIN_HEIGHT))
    }

    /// The saved position, or `None` when the title bar cannot be grabbed on any monitor.
    ///
    /// The strip's width is the logical width, which is never more than the physical width, so a high scale
    /// factor only makes the check stricter.
    pub fn visible_position(&self, monitors: &[MonitorRect]) -> Option<(i32, i32)> {
        if self.maximized {
            return self.maximized_corner_on_screen(monitors);
        }
        let (strip_width, _) = self.clamped_size();
        let strip_left = i64::from(self.x);
        let strip_right = strip_left + i64::from(strip_width);
        let strip_top = i64::from(self.y);
        let strip_bottom = strip_top + GRAB_STRIP_HEIGHT;
        let grabbable = monitors.iter().any(|monitor| {
            let monitor_left = i64::from(monitor.x);
            let monitor_top = i64::from(monitor.y);
            let overlap_width = strip_right.min(monitor_left + i64::from(monitor.width))
                - strip_left.max(monitor_left);
            let overlap_height = strip_bottom.min(monitor_top + i64::from(monitor.height))
                - strip_top.max(monitor_top);
            overlap_width >= GRAB_STRIP_MIN_WIDTH && overlap_height >= GRAB_STRIP_HEIGHT
        });
        if grabbable {
            Some((self.x, self.y))
        } else {
            None
        }
    }

    fn maximized_corner_on_screen(&self, monitors: &[MonitorRect]) -> Option<(i32, i32)> {
        let corner_x = i64::from(self.x) + MAXIMIZED_BORDER_TOLERANCE;
        let corner_y = i64::from(self.y) + MAXIMIZED_BORDER_TOLERANCE;
        let on_a_monitor = monitors.iter().any(|monitor| {
            let left = i64::from(monitor.x);
            let top = i64::from(monitor.y);
            (left..left + i64::from(monitor.width)).contains(&corner_x)
                && (top..top + i64::from(monitor.height)).contains(&corner_y)
        });
        if on_a_monitor {
            Some((self.x, self.y))
        } else {
            None
        }
    }
}
