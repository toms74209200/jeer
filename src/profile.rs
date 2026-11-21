use crate::lexer;
use crate::parser;
use crate::steps;
use crate::steps::line_endings::UnixText;

#[derive(Debug, PartialEq, Eq)]
pub enum Profile {
    Whitespace,
    LexerRoundtrip,
    CstRoundtrip,
    Gjf,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejection {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Rejection {
    fn at(text: &str, offset: usize, message: String) -> Rejection {
        let before = &text[..offset];
        let line = before.matches('\n').count() + 1;
        let column = before
            .rfind('\n')
            .map_or(before.chars().count(), |n| before[n + 1..].chars().count())
            + 1;
        Rejection {
            line,
            column,
            message,
        }
    }
}

impl std::str::FromStr for Profile {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "whitespace" => Ok(Profile::Whitespace),
            "lexer-roundtrip" => Ok(Profile::LexerRoundtrip),
            "cst-roundtrip" => Ok(Profile::CstRoundtrip),
            "gjf" => Ok(Profile::Gjf),
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
                let unix = String::from(UnixText::from(source));
                Ok(steps.iter().fold(unix, |text, step| step(&text)))
            }
            Profile::LexerRoundtrip => {
                let tokens = lexer::scan(source)
                    .map_err(|e| Rejection::at(source, e.offset, e.kind.to_string()))?;
                Ok(tokens.iter().map(|t| &source[t.start..t.end]).collect())
            }
            Profile::CstRoundtrip => {
                let tree = parser::parse(source)
                    .map_err(|e| Rejection::at(source, e.offset, e.message))?;
                Ok(tree
                    .tokens()
                    .iter()
                    .map(|t| &source[t.start..t.end])
                    .collect())
            }
            Profile::Gjf => {
                let unix = String::from(UnixText::from(source));
                parser::parse(&unix).map_err(|e| Rejection::at(&unix, e.offset, e.message))?;
                Ok(unix)
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
                    line: 1,
                    column: 11,
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
    fn when_format_with_cst_roundtrip_then_returns_the_source_or_the_syntax_error() {
        let cases = [
            (
                "class A {\r\n  int x; // c\r\n}",
                Ok("class A {\r\n  int x; // c\r\n}".to_owned()),
            ),
            ("class A { int x = 1 }", Err((1, 21))),
        ];
        for (source, expected) in cases {
            let formatted = Profile::CstRoundtrip
                .format(source)
                .map_err(|r| (r.line, r.column));

            assert_eq!(formatted, expected, "{source:?}");
        }
    }

    #[test]
    fn when_format_with_gjf_then_returns_unix_text_of_valid_input_or_the_syntax_error() {
        let cases = [
            ("class A {}\r\n", Ok("class A {}\n".to_owned())),
            ("class A {}\r", Ok("class A {}\n".to_owned())),
            ("class A {", Err((1, 10))),
            ("class A {\r\nvoid é;\r\n}\r\n", Err((2, 7))),
            ("class A {\r\n\r\n\r\n  int x = 1 }", Err((4, 13))),
        ];
        for (source, expected) in cases {
            let formatted = Profile::Gjf.format(source).map_err(|r| (r.line, r.column));

            assert_eq!(formatted, expected, "{source:?}");
        }
    }

    #[test]
    fn when_from_str_with_a_name_then_returns_its_profile_or_an_error() {
        let cases = [
            ("whitespace", Ok(Profile::Whitespace)),
            ("lexer-roundtrip", Ok(Profile::LexerRoundtrip)),
            ("cst-roundtrip", Ok(Profile::CstRoundtrip)),
            ("gjf", Ok(Profile::Gjf)),
            ("palantir", Err("unknown profile: palantir".to_owned())),
        ];
        for (name, expected) in cases {
            let profile = name.parse::<Profile>();

            assert_eq!(profile, expected, "{name}");
        }
    }
}
