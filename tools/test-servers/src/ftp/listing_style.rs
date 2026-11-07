/// The line format of the fake console's directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListingStyle {
    /// Aurora's FtpDll: a Unix long format with fixed owner, 13-column sizes and its own dates.
    #[default]
    Aurora,
    Unix,
    Dos,
    /// `MLSD` answers in machine format; `LIST` stays Unix.
    Mlsd,
}
