use crate::pipeline::Rejection;

pub fn apply(text: &str) -> Result<String, Rejection> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_apply_with_whitespace_at_line_ends_then_returns_only_spaces_and_tabs_removed() {
        let cases = [
            ("a  \nb\t\n", "a\nb\n"),
            ("a \t \n\t\n", "a\n\n"),
            ("a  ", "a"),
            ("a \u{c}\nb\u{b}\nc\u{a0}\n", "a \u{c}\nb\u{b}\nc\u{a0}\n"),
            ("a \u{2028}b", "a \u{2028}b"),
            ("", ""),
        ];
        for (text, expected) in cases {
            let trimmed = apply(text);

            assert_eq!(trimmed, Ok(expected.to_owned()), "{text:?}");
        }
    }
}
