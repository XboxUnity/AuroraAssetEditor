use super::SCREENSHOT_COUNT;

/// A screenshot position, 1 to 20, as the app numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenshotNumber(u8);

impl ScreenshotNumber {
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
