use chrono::{DateTime, Local};

use super::Clock;

#[derive(Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}
