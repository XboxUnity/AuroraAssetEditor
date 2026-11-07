/// The old TitleID and DatabaseID boxes' `MaxLength="8"`.
const HEX_INPUT_MAX_LEN: usize = 8;

/// The old boxes' input rule: `Regex.Replace(text, "[^a-fA-F0-9]+", "")`, then at most eight characters.
pub fn filter_hex_input(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_hexdigit)
        .take(HEX_INPUT_MAX_LEN)
        .collect()
}
