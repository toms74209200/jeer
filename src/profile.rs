use crate::steps;

#[derive(Debug, PartialEq, Eq)]
pub enum Profile {
    Whitespace,
}

impl std::str::FromStr for Profile {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "whitespace" => Ok(Profile::Whitespace),
            other => Err(format!("unknown profile: {other}")),
        }
    }
}

impl Profile {
    pub fn format(self, source: &str) -> String {
        let steps: &[fn(&str) -> String] = match self {
            Profile::Whitespace => &[
                steps::trim_trailing_whitespace::apply,
                steps::end_with_newline::apply,
            ],
        };
        let unix = if source.contains("\r\n") {
            source.replace('\r', "")
        } else {
            source.replace('\r', "\n")
        };
        steps.iter().fold(unix, |text, step| step(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_format_with_whitespace_then_returns_unix_text_trimmed_with_one_final_newline() {
        let cases = [
            ("class A {  \r\n}\r\n\r\n", "class A {\n}\n"),
            ("", "\n"),
            ("a\t\n \n", "a\n"),
            ("a\r\nb\rc", "a\nbc\n"),
            ("a\rb\r", "a\nb\n"),
        ];
        for (source, expected) in cases {
            let formatted = Profile::Whitespace.format(source);

            assert_eq!(formatted, expected, "{source:?}");
        }
    }

    #[test]
    fn when_from_str_with_a_name_then_returns_its_profile_or_an_error() {
        let cases = [
            ("whitespace", Ok(Profile::Whitespace)),
            ("gjf", Err("unknown profile: gjf".to_owned())),
        ];
        for (name, expected) in cases {
            let profile = name.parse::<Profile>();

            assert_eq!(profile, expected, "{name}");
        }
    }
}
