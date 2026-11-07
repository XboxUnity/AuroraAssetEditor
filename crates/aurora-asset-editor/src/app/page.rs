/// The sidebar's pages: the four editor pages, then the two sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Cover,
    Background,
    IconBanner,
    Screenshots,
    Online,
    Console,
}
