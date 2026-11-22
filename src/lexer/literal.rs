use super::scan;

pub fn string_value(literal: &[char]) -> String {
    let end = literal.len().saturating_sub(1);
    let mut value = String::new();
    let mut index = 1;
    while index < end {
        match literal[index] {
            '\\' => {
                let (c, next) = scan::escape_value(literal, index);
                value.push(c);
                index = next;
            }
            c => {
                value.push(c);
                index += 1;
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_string_value_with_escapes_then_returns_the_decoded_value() {
        let cases = [
            (r#""a""#, "a"),
            (r#""\45d""#, "%d"),
            (r#""\101\7\377x""#, "A\u{7}\u{ff}x"),
            (r#""\b\s\t\n\f\r\"\'\\""#, "\u{8} \t\n\u{c}\r\"'\\"),
        ];
        for (literal, expected) in cases {
            let chars: Vec<char> = literal.chars().collect();

            let value = string_value(&chars);

            assert_eq!(value, expected, "{literal}");
        }
    }
}
