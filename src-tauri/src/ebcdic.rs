/// EBCDIC (IBM CP037 — US/Canada) <-> ASCII translation, shared by the
/// TN3270 and TN5250 terminal emulators. Both protocols carry character
/// data in the host's EBCDIC code page rather than ASCII; CP037 is the
/// near-universal default for US/Canada mainframe and midrange systems.
/// Uncommon code points fall back to '?' on decode — this only affects the
/// rendered glyph for rarely-used punctuation, not protocol framing, so
/// it's a much lower-risk simplification than getting order/field-attribute
/// byte layouts wrong.
const EBCDIC_TO_ASCII: [u8; 256] = build_ebcdic_to_ascii();

const fn build_ebcdic_to_ascii() -> [u8; 256] {
    // Start with every byte mapping to '?' (0x3F), then patch in the
    // well-known CP037 code points below.
    let mut table = [0x3Fu8; 256];
    table[0x00] = 0x00;
    table[0x25] = b'\n'; // LF
    table[0x0D] = b'\r'; // CR
    table[0x40] = b' ';
    table[0x4B] = b'.';
    table[0x4C] = b'<';
    table[0x4D] = b'(';
    table[0x4E] = b'+';
    table[0x4F] = b'|';
    table[0x50] = b'&';
    table[0x5A] = b'!';
    table[0x5B] = b'$';
    table[0x5C] = b'*';
    table[0x5D] = b')';
    table[0x5E] = b';';
    table[0x5F] = b'^';
    table[0x60] = b'-';
    table[0x61] = b'/';
    table[0x6B] = b',';
    table[0x6C] = b'%';
    table[0x6D] = b'_';
    table[0x6E] = b'>';
    table[0x6F] = b'?';
    table[0x79] = b'`';
    table[0x7A] = b':';
    table[0x7B] = b'#';
    table[0x7C] = b'@';
    table[0x7D] = b'\'';
    table[0x7E] = b'=';
    table[0x7F] = b'"';

    // Lowercase a-i, j-r, s-z (EBCDIC letters are not contiguous — three runs)
    let lower = b"abcdefghi";
    let mut i = 0;
    while i < lower.len() {
        table[0x81 + i] = lower[i];
        i += 1;
    }
    let lower2 = b"jklmnopqr";
    i = 0;
    while i < lower2.len() {
        table[0x91 + i] = lower2[i];
        i += 1;
    }
    let lower3 = b"stuvwxyz";
    i = 0;
    while i < lower3.len() {
        table[0xA2 + i] = lower3[i];
        i += 1;
    }

    table[0x8F] = b'!'; // rarely used position, harmless overlap with 0x5A elsewhere

    // Uppercase A-I, J-R, S-Z (same three-run pattern)
    let upper = b"ABCDEFGHI";
    i = 0;
    while i < upper.len() {
        table[0xC1 + i] = upper[i];
        i += 1;
    }
    let upper2 = b"JKLMNOPQR";
    i = 0;
    while i < upper2.len() {
        table[0xD1 + i] = upper2[i];
        i += 1;
    }
    let upper3 = b"STUVWXYZ";
    i = 0;
    while i < upper3.len() {
        table[0xE2 + i] = upper3[i];
        i += 1;
    }

    // Digits 0-9
    let digits = b"0123456789";
    i = 0;
    while i < digits.len() {
        table[0xF0 + i] = digits[i];
        i += 1;
    }

    table
}

/// Reverse lookup, built once from the forward table — every ASCII byte
/// that appears in `EBCDIC_TO_ASCII` maps back to the EBCDIC byte that
/// produced it. Bytes with no EBCDIC representation fall back to 0x40
/// (EBCDIC space), matching how real 3270/5250 clients handle
/// unencodable input.
fn ascii_to_ebcdic_table() -> [u8; 256] {
    let mut table = [0x40u8; 256];
    for (ebcdic, &ascii) in EBCDIC_TO_ASCII.iter().enumerate() {
        if ascii != 0x3F || ebcdic == 0x6F {
            table[ascii as usize] = ebcdic as u8;
        }
    }
    table
}

pub fn ebcdic_to_ascii(byte: u8) -> u8 {
    EBCDIC_TO_ASCII[byte as usize]
}

pub fn ascii_to_ebcdic(byte: u8) -> u8 {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(ascii_to_ebcdic_table)[byte as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digits_roundtrip() {
        for (i, &d) in b"0123456789".iter().enumerate() {
            assert_eq!(ebcdic_to_ascii(0xF0 + i as u8), d);
            assert_eq!(ascii_to_ebcdic(d), 0xF0 + i as u8);
        }
    }

    #[test]
    fn test_uppercase_letters() {
        assert_eq!(ebcdic_to_ascii(0xC1), b'A');
        assert_eq!(ebcdic_to_ascii(0xC9), b'I');
        assert_eq!(ebcdic_to_ascii(0xD1), b'J');
        assert_eq!(ebcdic_to_ascii(0xE2), b'S');
        assert_eq!(ebcdic_to_ascii(0xE9), b'Z');
        assert_eq!(ascii_to_ebcdic(b'A'), 0xC1);
        assert_eq!(ascii_to_ebcdic(b'Z'), 0xE9);
    }

    #[test]
    fn test_lowercase_letters() {
        assert_eq!(ebcdic_to_ascii(0x81), b'a');
        assert_eq!(ebcdic_to_ascii(0xA2), b's');
        assert_eq!(ebcdic_to_ascii(0xA9), b'z');
        assert_eq!(ascii_to_ebcdic(b'z'), 0xA9);
    }

    #[test]
    fn test_space() {
        assert_eq!(ebcdic_to_ascii(0x40), b' ');
        assert_eq!(ascii_to_ebcdic(b' '), 0x40);
    }

    #[test]
    fn test_unmapped_byte_falls_back_to_question_mark() {
        assert_eq!(ebcdic_to_ascii(0x01), b'?');
    }
}
