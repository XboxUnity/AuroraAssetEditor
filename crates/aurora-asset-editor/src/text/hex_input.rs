const HEX_INPUT_MAX_LEN: usize = 8;

/// Keeps the hexadecimal digits of `text`, then at most eight characters.
pub fn filter_hex_input(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_hexdigit)
        .take(HEX_INPUT_MAX_LEN)
        .collect()
}
