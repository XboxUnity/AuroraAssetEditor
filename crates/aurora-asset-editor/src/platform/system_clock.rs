use chrono::{DateTime, Local};

use super::Clock;

/// The system's local time.
#[derive(Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}
