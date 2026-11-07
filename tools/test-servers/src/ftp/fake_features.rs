/// What the fake console answers to `FEAT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeFeatures {
    /// `500`: the console does not know the command.
    Refused,
    /// A multi-line `211` advertising `SIZE` only.
    Basic,
    /// A multi-line `211` advertising `MLSD` and `UTF8`.
    MlsdAndUtf8,
}

impl FakeFeatures {
    /// The info lines between `211-Features:` and `211 End`; `None` when `FEAT` is refused.
    pub(crate) fn info_lines(self) -> Option<&'static [&'static str]> {
        match self {
            FakeFeatures::Refused => None,
            FakeFeatures::Basic => Some(&[" SIZE"]),
            FakeFeatures::MlsdAndUtf8 => Some(&[" MLSD", " UTF8"]),
        }
    }
}
