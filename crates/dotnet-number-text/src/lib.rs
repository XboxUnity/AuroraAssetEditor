#![forbid(unsafe_code)]

/// Trims number text the way .NET's `int.TryParse` and `uint.TryParse` do for
/// `NumberStyles.Integer` and `NumberStyles.HexNumber`: white space (U+0009 to U+000D and U+0020
/// only, not other Unicode white space) at both ends, with trailing NUL characters allowed only
/// after the trailing white space. Both the settings port and the TitleID box must accept exactly
/// the same text, so both call this.
pub fn trim_number_text(text: &str) -> &str {
    text.trim_end_matches('\0')
        .trim_matches(is_number_white_space)
}

fn is_number_white_space(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_white_space_at_both_ends() {
        assert_eq!(trim_number_text("\t\n1F\r "), "1F");
    }

    #[test]
    fn allows_trailing_nuls_after_the_white_space() {
        assert_eq!(trim_number_text("1F\0"), "1F");
        assert_eq!(trim_number_text("1F \0"), "1F");
    }

    #[test]
    fn leaves_a_nul_that_is_followed_by_white_space() {
        assert_eq!(trim_number_text("1F\0 "), "1F\0");
        assert_eq!(trim_number_text("1F \0 "), "1F \0");
    }

    #[test]
    fn does_not_trim_other_unicode_white_space() {
        assert_eq!(trim_number_text("\u{a0}1F"), "\u{a0}1F");
    }
}
