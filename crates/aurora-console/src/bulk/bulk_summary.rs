use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BulkSummary {
    pub uploaded: usize,
    pub already_had_cover: usize,
    pub unreadable: usize,
    pub no_cover_online: usize,
    pub failed: usize,
}

impl fmt::Display for BulkSummary {
    /// Reads like "3 covers uploaded, 2 already had one, 1 unreadable cover skipped,
    /// 1 without a cover online, 1 failed".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} uploaded, {} already had one, {} unreadable {} skipped, \
             {} without a cover online, {} failed",
            self.uploaded,
            cover_noun(self.uploaded),
            self.already_had_cover,
            self.unreadable,
            cover_noun(self.unreadable),
            self.no_cover_online,
            self.failed,
        )
    }
}

fn cover_noun(count: usize) -> &'static str {
    if count == 1 { "cover" } else { "covers" }
}
