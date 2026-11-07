/// The line format of the fake console's directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingStyle {
    Unix,
    Dos,
    /// `MLSD` answers in machine format; `LIST` stays Unix.
    Mlsd,
}
