const UNRESERVED_PUNCTUATION: &[u8] = b"-_.!*()";
const UPPER_HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
const NIBBLE_BITS: u32 = 4;
const NIBBLE_MASK: u8 = 0x0F;

/// Same output as .NET `WebUtility.UrlEncode`: a space is `+`, `'` and `~` are escaped.
pub(crate) fn url_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || UNRESERVED_PUNCTUATION.contains(&byte) {
            encoded.push(char::from(byte));
        } else if byte == b' ' {
            encoded.push('+');
        } else {
            encoded.push('%');
            encoded.push(char::from(
                UPPER_HEX_DIGITS[usize::from(byte >> NIBBLE_BITS)],
            ));
            encoded.push(char::from(
                UPPER_HEX_DIGITS[usize::from(byte & NIBBLE_MASK)],
            ));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_matches_webutility() {
        assert_eq!(
            url_encode("halo 3/é'~!*()-_.+&?"),
            "halo+3%2F%C3%A9%27%7E!*()-_.%2B%26%3F"
        );
    }
}
