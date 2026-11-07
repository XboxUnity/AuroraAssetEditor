use serde::{Deserialize, Serialize};

pub const MIN_WIDTH: u32 = 760;
pub const MIN_HEIGHT: u32 = 500;

const GRAB_STRIP_HEIGHT: i64 = 32;
const GRAB_STRIP_MIN_WIDTH: i64 = 64;
const DEFAULT_WIDTH: u32 = 1024;
const DEFAULT_HEIGHT: u32 = 680;

/// `x` and `y` are physical pixels (winit `outer_position`); `width` and `height` are logical pixels (Slint's window size).
/// For a maximized window they are the position and size it returns to. Missing fields take the default window
/// size, position 0,0 and not maximized.
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
    /// What to save, with `self` the geometry saved before, for a window that is `maximized` now, `size` big and
    /// at `position` (`None` when the system does not report it). A maximized window keeps the saved position
    /// and size, which are where it returns to.
    pub fn captured(
        &self,
        maximized: bool,
        size: (u32, u32),
        position: Option<(i32, i32)>,
    ) -> WindowGeometry {
        if maximized {
            return WindowGeometry {
                maximized: true,
                ..*self
            };
        }
        let (x, y) = position.unwrap_or((self.x, self.y));
        let (width, height) = size;
        WindowGeometry {
            x,
            y,
            width,
            height,
            maximized: false,
        }
    }

    pub fn clamped_size(&self) -> (u32, u32) {
        (self.width.max(MIN_WIDTH), self.height.max(MIN_HEIGHT))
    }

    /// The saved position, or `None` when the title bar cannot be grabbed on any monitor.
    ///
    /// The strip's width is the logical width, which is never more than the physical width, so a high scale
    /// factor only makes the check stricter.
    pub fn visible_position(&self, monitors: &[MonitorRect]) -> Option<(i32, i32)> {
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
}
