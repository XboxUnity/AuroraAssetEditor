/// What the fake console answers to `FEAT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FakeFeatures {
    /// `500`: the console does not know the command.
    Refused,
    /// Aurora's FtpDll: `XCRC filename`, `CDUP` and `UTF8`. It advertises `UTF8` yet answers
    /// `OPTS UTF8 ON` with `502 Command not found`.
    #[default]
    Aurora,
    /// A multi-line `211` advertising `SIZE` only.
    Basic,
    /// A multi-line `211` advertising `MLSD` and `UTF8`; `OPTS UTF8 ON` is accepted.
    MlsdAndUtf8,
}

impl FakeFeatures {
    /// The info lines between `211-Extensions supported:` and `211 END`; `None` when `FEAT` is refused.
    pub(crate) fn info_lines(self) -> Option<&'static [&'static str]> {
        match self {
            FakeFeatures::Refused => None,
            FakeFeatures::Aurora => Some(&[" XCRC filename", " CDUP", " UTF8"]),
            FakeFeatures::Basic => Some(&[" SIZE"]),
            FakeFeatures::MlsdAndUtf8 => Some(&[" MLSD", " UTF8"]),
        }
    }

    pub(crate) fn advertises_mlsd(self) -> bool {
        self.info_lines()
            .is_some_and(|lines| lines.iter().any(|line| line.trim() == "MLSD"))
    }

    pub(crate) fn accepts_utf8_option(self) -> bool {
        self != FakeFeatures::Aurora
    }
}
