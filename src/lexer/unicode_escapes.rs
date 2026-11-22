use super::token::{LexError, LexErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translated {
    pub chars: Vec<char>,
    pub offsets: Vec<usize>,
}

impl Translated {
    pub fn decoded(&self, start: usize, end: usize) -> String {
        let from = self.offsets.partition_point(|&offset| offset < start);
        let to = self.offsets.partition_point(|&offset| offset < end);
        self.chars[from..to.max(from)].iter().collect()
    }
}

pub fn translate(source: &str) -> Result<Translated, LexError> {
    let bytes = source.as_bytes();
    let mut units: Vec<(u32, usize)> = Vec::with_capacity(source.len());
    let mut raw_backslashes = 0usize;
    let mut position = 0;
    while position < source.len() {
        let eligible = bytes[position] == b'\\' && raw_backslashes.is_multiple_of(2);
        let mut after_us = position + 1;
        while eligible && after_us < bytes.len() && bytes[after_us] == b'u' {
            after_us += 1;
        }
        if eligible && after_us > position + 1 {
            let unit = source
                .get(after_us..after_us + 4)
                .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
                .and_then(|d| u32::from_str_radix(d, 16).ok())
                .ok_or(LexError {
                    kind: LexErrorKind::IllegalUnicodeEscape,
                    offset: position,
                })?;
            units.push((unit, position));
            raw_backslashes = 0;
            position = after_us + 4;
        } else {
            let c = source[position..].chars().next().unwrap_or('\u{fffd}');
            raw_backslashes = if c == '\\' { raw_backslashes + 1 } else { 0 };
            units.push((u32::from(c), position));
            position += c.len_utf8();
        }
    }
    let mut chars = Vec::with_capacity(units.len());
    let mut offsets = Vec::with_capacity(units.len() + 1);
    let mut index = 0;
    while index < units.len() {
        let (unit, offset) = units[index];
        let low = units.get(index + 1).map_or(0, |u| u.0);
        let (c, width) = if (0xD800..0xDC00).contains(&unit) && (0xDC00..0xE000).contains(&low) {
            let combined = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
            (char::from_u32(combined).unwrap_or('\u{fffd}'), 2)
        } else {
            (char::from_u32(unit).unwrap_or('\u{fffd}'), 1)
        };
        chars.push(c);
        offsets.push(offset);
        index += width;
    }
    offsets.push(source.len());
    Ok(Translated { chars, offsets })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_translate_with_unicode_escapes_then_returns_the_chars_with_their_source_offsets() {
        let cases: [(&str, &str, &[usize]); 7] = [
            ("ab", "ab", &[0, 1, 2]),
            ("\\u0041b", "Ab", &[0, 6, 7]),
            ("\\uuu0041", "A", &[0, 8]),
            ("\\\\u0041", "\\\\u0041", &[0, 1, 2, 3, 4, 5, 6, 7]),
            ("\\\\\\u0041", "\\\\A", &[0, 1, 2, 8]),
            ("\\u005c\\u0041", "\\A", &[0, 6, 12]),
            ("\\uD83D\\uDE00x", "\u{1F600}x", &[0, 12, 13]),
        ];
        for (source, chars, offsets) in cases {
            let translated =
                translate(source).map(|t| (t.chars.iter().collect::<String>(), t.offsets));

            assert_eq!(
                translated,
                Ok((chars.to_owned(), offsets.to_vec())),
                "{source:?}"
            );
        }
    }

    #[test]
    fn when_translate_with_non_ascii_text_then_returns_byte_offsets() {
        let source = "é\\u00e9";

        let translated = translate(source).map(|t| (t.chars, t.offsets));

        assert_eq!(translated, Ok((vec!['é', 'é'], vec![0, 2, 8])));
    }

    #[test]
    fn when_translate_with_a_lone_surrogate_then_returns_a_replacement_char() {
        let source = "\\uD800a";

        let translated = translate(source).map(|t| t.chars);

        assert_eq!(translated, Ok(vec!['\u{fffd}', 'a']));
    }

    #[test]
    fn when_translate_with_a_malformed_escape_then_returns_illegal_unicode_escape() {
        let cases = [("a\\u12", 1), ("\\uXYZW", 0), ("\\uu12g4", 0)];
        for (source, offset) in cases {
            let translated = translate(source).err();

            assert_eq!(
                translated,
                Some(LexError {
                    kind: LexErrorKind::IllegalUnicodeEscape,
                    offset
                }),
                "{source:?}"
            );
        }
    }
}
