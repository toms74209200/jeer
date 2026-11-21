use crate::lexer;
use crate::steps;

#[derive(Debug, PartialEq, Eq)]
pub enum Profile {
    Whitespace,
    LexerRoundtrip,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejection {
    pub offset: usize,
    pub message: String,
}

impl std::str::FromStr for Profile {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "whitespace" => Ok(Profile::Whitespace),
            "lexer-roundtrip" => Ok(Profile::LexerRoundtrip),
            other => Err(format!("unknown profile: {other}")),
        }
    }
}

impl Profile {
    pub fn format(self, source: &str) -> Result<String, Rejection> {
        match self {
            Profile::Whitespace => {
                let steps: [fn(&str) -> String; 2] = [
                    steps::trim_trailing_whitespace::apply,
                    steps::end_with_newline::apply,
                ];
                let unix = if source.contains("\r\n") {
                    source.replace('\r', "")
                } else {
                    source.replace('\r', "\n")
                };
                Ok(steps.iter().fold(unix, |text, step| step(&text)))
            }
            Profile::LexerRoundtrip => {
                let tokens = lexer::scan(source).map_err(|e| Rejection {
                    offset: e.offset,
                    message: e.kind.to_string(),
                })?;
                Ok(tokens.iter().map(|t| &source[t.start..t.end]).collect())
            }
        }
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

            assert_eq!(formatted, Ok(expected.to_owned()), "{source:?}");
        }
    }

    #[test]
    fn when_format_with_lexer_roundtrip_then_returns_the_source_or_the_lexical_error() {
        let cases = [
            (
                "class A {\r\n  int x = 0x1F; // c\r\n}",
                Ok("class A {\r\n  int x = 0x1F; // c\r\n}".to_owned()),
            ),
            (
                "class A { /* open",
                Err(Rejection {
                    offset: 10,
                    message: "unclosed comment".to_owned(),
                }),
            ),
        ];
        for (source, expected) in cases {
            let formatted = Profile::LexerRoundtrip.format(source);

            assert_eq!(formatted, expected, "{source:?}");
        }
    }

    #[test]
    fn when_from_str_with_a_name_then_returns_its_profile_or_an_error() {
        let cases = [
            ("whitespace", Ok(Profile::Whitespace)),
            ("lexer-roundtrip", Ok(Profile::LexerRoundtrip)),
            ("gjf", Err("unknown profile: gjf".to_owned())),
        ];
        for (name, expected) in cases {
            let profile = name.parse::<Profile>();

            assert_eq!(profile, expected, "{name}");
        }
    }
}
