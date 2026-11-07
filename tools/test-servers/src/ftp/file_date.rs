/// A file's modification time as the fake console lists it. The month is 1 to 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

/// The fake console's own clock, which decides whether a listing shows the time or the year.
pub(crate) const FAKE_NOW: FileDate = FileDate {
    year: 2026,
    month: 11,
    day: 7,
    hour: 12,
    minute: 0,
};

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Aurora compares `now` with the file's 0-based month, so "recent" means fewer than 5 full months.
const RECENT_MONTH_DIFFERENCE: i32 = 6;

impl FileDate {
    /// Aurora's date column: `Nov 03 14:05` when recent, `Jan 05 2019` otherwise, day zero-padded.
    pub(crate) fn aurora_text(&self) -> String {
        let month = MONTH_NAMES[(self.month - 1) as usize];
        let month_difference =
            (FAKE_NOW.year * 12 + FAKE_NOW.month as i32) - (self.year * 12 + self.month as i32 - 1);
        if month_difference < RECENT_MONTH_DIFFERENCE {
            format!(
                "{month} {:02} {:02}:{:02}",
                self.day, self.hour, self.minute
            )
        } else {
            format!("{month} {:02} {}", self.day, self.year)
        }
    }
}
