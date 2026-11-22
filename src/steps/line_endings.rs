use crate::pipeline::Rejection;

pub fn apply(text: &str) -> Result<String, Rejection> {
    let unix = match text.contains("\r\n") {
        true => text.replace('\r', ""),
        false => text.replace('\r', "\n"),
    };
    Ok(unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_apply_with_each_kind_of_line_ending_then_returns_it_as_spotless_does() {
        let cases = [
            ("a\nb\n", "a\nb\n"),
            ("a\r\nb\r\n", "a\nb\n"),
            ("a\rb\r", "a\nb\n"),
            ("a\r\nb\rc", "a\nbc"),
            ("", ""),
        ];
        for (text, expected) in cases {
            let unix = apply(text);

            assert_eq!(unix, Ok(expected.to_owned()), "{text:?}");
        }
    }
}
