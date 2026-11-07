use super::{ENTRY_COUNT, FIRST_SCREENSHOT_INDEX, ScreenshotNumber};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetSlot {
    Icon,
    Banner,
    Cover,
    Slot,
    Background,
    Screenshot(ScreenshotNumber),
}

impl AssetSlot {
    /// The entry number in the file; with `from_index`, the only place the numbering lives.
    pub fn index(self) -> usize {
        match self {
            Self::Icon => 0,
            Self::Banner => 1,
            Self::Cover => 2,
            Self::Slot => 3,
            Self::Background => 4,
            Self::Screenshot(number) => FIRST_SCREENSHOT_INDEX + number.position(),
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Icon),
            1 => Some(Self::Banner),
            2 => Some(Self::Cover),
            3 => Some(Self::Slot),
            4 => Some(Self::Background),
            FIRST_SCREENSHOT_INDEX..ENTRY_COUNT => {
                let number = (index - FIRST_SCREENSHOT_INDEX + 1) as u8;
                ScreenshotNumber::new(number).map(Self::Screenshot)
            }
            _ => None,
        }
    }

    pub fn screenshot(number: u8) -> Option<Self> {
        ScreenshotNumber::new(number).map(Self::Screenshot)
    }
}
