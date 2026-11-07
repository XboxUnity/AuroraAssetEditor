use super::SCREENSHOT_COUNT;

/// A screenshot position, 1 to 20, as the app numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenshotNumber(u8);

impl ScreenshotNumber {
    pub const ALL: [ScreenshotNumber; SCREENSHOT_COUNT] = {
        let mut all = [Self(1); SCREENSHOT_COUNT];
        let mut position = 0;
        while position < SCREENSHOT_COUNT {
            all[position] = Self(position as u8 + 1);
            position += 1;
        }
        all
    };

    pub fn new(number: u8) -> Option<Self> {
        if (1..=SCREENSHOT_COUNT as u8).contains(&number) {
            Some(Self(number))
        } else {
            None
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub(super) fn position(self) -> usize {
        usize::from(self.0) - 1
    }
}
