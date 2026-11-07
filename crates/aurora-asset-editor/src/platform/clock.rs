use chrono::{DateTime, Local};

pub trait Clock {
    fn now(&self) -> DateTime<Local>;
}
