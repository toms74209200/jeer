use crate::pipeline::Rejection;

pub fn apply(text: &str) -> Result<String, Rejection> {
    let content = text.trim_end_matches([' ', '\t', '\n']);
    Ok(format!("{content}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_apply_with_any_ending_then_returns_the_content_with_one_newline() {
        let cases = [
            ("a", "a\n"),
            ("a\n", "a\n"),
            ("a\n\n \t\n", "a\n"),
            ("a\u{c}", "a\u{c}\n"),
            ("a\n\u{b}", "a\n\u{b}\n"),
            ("", "\n"),
        ];
        for (text, expected) in cases {
            let ended = apply(text);

            assert_eq!(ended, Ok(expected.to_owned()), "{text:?}");
        }
    }
}
