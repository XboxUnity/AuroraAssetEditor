use aurora_asset_editor::platform::Clock;
use chrono::{DateTime, FixedOffset, Local, NaiveDate};

/// Always the same moment. The offset is fixed too, so wall-clock texts do not depend on the machine's
/// time zone.
pub struct FixedClock {
    now: DateTime<Local>,
}

impl FixedClock {
    pub fn new(now: DateTime<FixedOffset>) -> FixedClock {
        FixedClock {
            now: DateTime::from_naive_utc_and_offset(now.naive_utc(), *now.offset()),
        }
    }

    pub fn harness_time() -> FixedClock {
        let offset = FixedOffset::east_opt(3600).expect("one hour is a valid offset");
        let now = NaiveDate::from_ymd_opt(2026, 11, 7)
            .and_then(|date| date.and_hms_opt(20, 41, 5))
            .expect("2026-11-07 20:41:05 is a valid date and time")
            .and_local_timezone(offset)
            .single()
            .expect("a fixed offset maps every local time once");
        FixedClock::new(now)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Local> {
        self.now
    }
}
