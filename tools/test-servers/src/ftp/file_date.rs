use std::fmt;

/// A file's modification time as the fake console lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDate {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

/// A [`FileDate`] field outside its calendar range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFileDate {
    field: &'static str,
}

impl fmt::Display for InvalidFileDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the {} of a file date is out of range", self.field)
    }
}

impl std::error::Error for InvalidFileDate {}

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
    /// The month is 1 to 12, the day 1 to 31, the hour 0 to 23 and the minute 0 to 59.
    pub fn new(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    ) -> Result<FileDate, InvalidFileDate> {
        let checks = [
            ("month", (1..=12).contains(&month)),
            ("day", (1..=31).contains(&day)),
            ("hour", hour < 24),
            ("minute", minute < 60),
        ];
        if let Some((field, _)) = checks.iter().find(|(_, valid)| !valid) {
            return Err(InvalidFileDate { field });
        }
        Ok(FileDate {
            year,
            month,
            day,
            hour,
            minute,
        })
    }

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
