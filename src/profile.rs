use crate::gjf;
use crate::gjf::builder::Unformatted;
use crate::lexer;
use crate::parser;
use crate::steps;
use crate::steps::line_endings::UnixText;

#[derive(Debug, PartialEq, Eq)]
pub enum Profile {
    Whitespace,
    ImportOrder,
    UnusedImports,
    Toggle,
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
            "import-order" => Ok(Profile::ImportOrder),
            "unused-imports" => Ok(Profile::UnusedImports),
            "toggle" => Ok(Profile::Toggle),
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
            Profile::ImportOrder => {
                let unix = String::from(UnixText::from(source));
                steps::import_order::apply(&unix)
                    .map_err(|_| Rejection::at(&unix, 0, "import order failed".to_owned()))
            }
            Profile::UnusedImports => {
                let unix = String::from(UnixText::from(source));
                steps::unused_imports::apply(&unix)
                    .map_err(|e| Rejection::at(&unix, e.offset, e.message))
            }
            Profile::Toggle => {
                let unix = String::from(UnixText::from(source));
                steps::toggle::preserving(steps::trim_trailing_whitespace::apply)(&unix).map_err(
                    |_| {
                        Rejection::at(
                            &unix,
                            0,
                            "a step removed a spotless:off/on fence".to_owned(),
                        )
                    },
                )
            }
            Profile::LexerRoundtrip => {
                let tokens = lexer::scan(source)
                    .map_err(|e| Rejection::at(source, e.offset, e.kind.to_string()))?
                    .tokens;
                Ok(tokens.iter().map(|t| &source[t.start..t.end]).collect())
            }
            Profile::CstRoundtrip => {
                let tree = parser::parse(source)
                    .map_err(|e| Rejection::at(source, e.offset, e.message))?
                    .tree;
                Ok(tree
                    .tokens()
                    .iter()
                    .map(|t| &source[t.start..t.end])
                    .collect())
            }
            Profile::Gjf => {
                let unix = String::from(UnixText::from(source));
                let unit =
                    parser::parse(&unix).map_err(|e| Rejection::at(&unix, e.offset, e.message))?;
                match gjf::format(&unix, &unit) {
                    Ok(formatted) => Ok(formatted),
                    Err(Unformatted::Deferred) => Ok(unix),
                    Err(Unformatted::Rejected { offset, message }) => {
                        Err(Rejection::at(&unix, offset, message))
                    }
                }
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
    fn when_format_with_gjf_then_returns_formatted_or_kept_text_or_the_rejection() {
        let cases = [
            ("class A {}\r\n", Ok("class A {}\n".to_owned())),
            (
                "class A{int x;}\n",
                Ok("class A {\n  int x;\n}\n".to_owned()),
            ),
            (
                "import a.B;\r\nclass A{}\r\n",
                Ok("import a.B;\nclass A{}\n".to_owned()),
            ),
            ("class A { int \\u0061b; }\n", Err((1, 15))),
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
    fn when_format_with_a_step_profile_then_returns_the_step_output_or_the_rejection() {
        let cases = [
            (
                Profile::ImportOrder,
                "import b.B;\r\nimport a.A;\r\nclass A {}\r\n",
                Ok("import a.A;\nimport b.B;\nclass A {}\n".to_owned()),
            ),
            (Profile::ImportOrder, "  ", Err((1, 1))),
            (
                Profile::UnusedImports,
                "import a.B;\r\nclass A {}\r\n",
                Ok("class A {}\n".to_owned()),
            ),
            (
                Profile::UnusedImports,
                "import a.B;\nclass A {",
                Err((2, 10)),
            ),
            (
                Profile::Toggle,
                "a  \n// spotless:off\nb  \n// spotless:on\n",
                Ok("a\n// spotless:off\nb  \n// spotless:on\n".to_owned()),
            ),
        ];
        for (profile, source, expected) in cases {
            let formatted = profile.format(source).map_err(|r| (r.line, r.column));

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
            ("import-order", Ok(Profile::ImportOrder)),
            ("unused-imports", Ok(Profile::UnusedImports)),
            ("toggle", Ok(Profile::Toggle)),
            ("palantir", Err("unknown profile: palantir".to_owned())),
        ];
        for (name, expected) in cases {
            let profile = name.parse::<Profile>();

            assert_eq!(profile, expected, "{name}");
        }
    }
}
