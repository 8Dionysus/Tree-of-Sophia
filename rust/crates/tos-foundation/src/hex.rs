//! Lowercase hexadecimal encoding for byte sequences.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Encode one byte as two lowercase hexadecimal digits.
pub const fn encode_lower_hex_byte(byte: u8) -> [u8; 2] {
    [
        DIGITS[(byte >> 4) as usize],
        DIGITS[(byte & 15) as usize],
    ]
}

/// Encode bytes as lowercase hexadecimal digits.
pub fn encode_lower_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        let [high, low] = encode_lower_hex_byte(byte);
        text.push(high as char);
        text.push(low as char);
    }
    text
}

/// Check the exact digit count and lowercase hexadecimal alphabet.
pub fn is_lower_hex(value: &str, expected_digits: usize) -> bool {
    value.len() == expected_digits
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Decode one lowercase hexadecimal digit.
pub const fn decode_lower_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Decode lowercase hexadecimal into an exact-sized byte slice.
pub fn decode_lower_hex_into(value: &str, output: &mut [u8]) -> bool {
    if output.len().checked_mul(2) != Some(value.len()) || !is_lower_hex(value, value.len()) {
        return false;
    }
    for (out, pair) in output.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = decode_lower_hex_digit(pair[0]).expect("validated hex");
        let low = decode_lower_hex_digit(pair[1]).expect("validated hex");
        *out = (high << 4) | low;
    }
    true
}

/// Decode lowercase hexadecimal with an exact expected byte count.
pub fn decode_lower_hex(value: &str, expected_bytes: usize) -> Option<Vec<u8>> {
    if expected_bytes.checked_mul(2) != Some(value.len()) {
        return None;
    }
    let mut bytes = vec![0; expected_bytes];
    decode_lower_hex_into(value, &mut bytes).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_hex_roundtrip_and_validation() {
        let bytes = [0, 1, 15, 16, 127, 128, 255];
        let encoded = encode_lower_hex(&bytes);
        assert_eq!(encoded, "00010f107f80ff");
        assert_eq!(
            decode_lower_hex(&encoded, bytes.len()).as_deref(),
            Some(bytes.as_slice())
        );
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode_lower_hex(&encode_lower_hex(&all), all.len()), Some(all));
        assert_eq!(decode_lower_hex("", 0), Some(vec![]));
        assert!(is_lower_hex("abc123", 6));
        for invalid in ["ABC123", "abc12g", "abc12é", "abc12"] {
            assert!(!is_lower_hex(invalid, 6));
            assert_eq!(decode_lower_hex(invalid, 3), None);
        }
        assert_eq!(decode_lower_hex(&encoded, 8), None);
        assert_eq!(decode_lower_hex("0", usize::MAX), None);
    }
}
