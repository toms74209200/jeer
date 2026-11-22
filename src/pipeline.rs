use crate::gjf;
use crate::lexer;
use crate::parser;
use crate::steps;

#[derive(Debug, PartialEq, Eq)]
pub enum Position {
    Offset(usize),
    #[expect(
        dead_code,
        reason = "gjf reports errors after its cleanups at a line and column of the cleaned text in task 118"
    )]
    LineColumn {
        line: usize,
        column: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejection {
    pub position: Position,
    pub message: String,
}

pub type Step = Box<dyn Fn(&str) -> Result<String, Rejection>>;

#[derive(Debug, PartialEq, Eq)]
pub struct Located {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Located {
    fn at(text: &str, rejection: Rejection) -> Located {
        match rejection.position {
            Position::LineColumn { line, column } => Located {
                line,
                column,
                message: rejection.message,
            },
            Position::Offset(offset) => {
                let before = &text[..offset];
                let line = before.matches('\n').count() + 1;
                let column = before
                    .rfind('\n')
                    .map_or(before.chars().count(), |n| before[n + 1..].chars().count())
                    + 1;
                Located {
                    line,
                    column,
                    message: rejection.message,
                }
            }
        }
    }
}

pub fn compose(steps: Vec<Step>) -> impl Fn(&str) -> Result<String, Located> {
    move |source| {
        steps.iter().try_fold(source.to_owned(), |text, step| {
            step(&text).map_err(|rejection| Located::at(&text, rejection))
        })
    }
}

const MAX_CYCLE: usize = 10;

pub fn padded_cell<F>(format: F) -> impl Fn(&str) -> Result<String, Located>
where
    F: Fn(&str) -> Result<String, Located>,
{
    move |source| {
        let mut applied = vec![format(source)?];
        loop {
            let last = &applied[applied.len() - 1];
            if applied.len() == 1 && last == source {
                return Ok(source.to_owned());
            }
            if applied.len() == MAX_CYCLE {
                return Ok(source.to_owned());
            }
            let next = format(last)?;
            if next == *last {
                return Ok(next);
            }
            if let Some(start) = applied.iter().position(|text| *text == next) {
                let canonical = applied[start..].iter().min_by(|a, b| {
                    a.encode_utf16()
                        .count()
                        .cmp(&b.encode_utf16().count())
                        .then_with(|| a.encode_utf16().cmp(b.encode_utf16()))
                });
                return Ok(canonical.map_or(next.clone(), Clone::clone));
            }
            applied.push(next);
        }
    }
}

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
    fn steps(self) -> Vec<Step> {
        match self {
            Profile::Whitespace => vec![
                Box::new(steps::line_endings::apply),
                Box::new(steps::trim_trailing_whitespace::apply),
                Box::new(steps::end_with_newline::apply),
            ],
            Profile::ImportOrder => vec![
                Box::new(steps::line_endings::apply),
                Box::new(steps::import_order::apply),
            ],
            Profile::UnusedImports => vec![
                Box::new(steps::line_endings::apply),
                Box::new(steps::unused_imports::apply),
            ],
            Profile::Toggle => vec![
                Box::new(steps::line_endings::apply),
                Box::new(steps::toggle::preserving(
                    steps::trim_trailing_whitespace::apply,
                )),
            ],
            Profile::LexerRoundtrip => vec![Box::new(|source: &str| {
                lexer::scan(source)
                    .map(|lexed| {
                        lexed
                            .tokens
                            .iter()
                            .map(|t| &source[t.start..t.end])
                            .collect()
                    })
                    .map_err(|e| Rejection {
                        position: Position::Offset(e.offset),
                        message: e.kind.to_string(),
                    })
            })],
            Profile::CstRoundtrip => vec![Box::new(|source: &str| {
                parser::parse(source)
                    .map(|unit| {
                        unit.tree
                            .tokens()
                            .iter()
                            .map(|t| &source[t.start..t.end])
                            .collect()
                    })
                    .map_err(|e| Rejection {
                        position: Position::Offset(e.offset),
                        message: e.message,
                    })
            })],
            Profile::Gjf => vec![
                Box::new(steps::line_endings::apply),
                Box::new(gjf::step(gjf::Options::default())),
            ],
        }
    }
}

impl Profile {
    pub fn pipeline(self) -> impl Fn(&str) -> Result<String, Located> {
        padded_cell(compose(self.steps()))
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
            let formatted = Profile::Whitespace.pipeline()(source);

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
                Err(Located {
                    line: 1,
                    column: 11,
                    message: "unclosed comment".to_owned(),
                }),
            ),
        ];
        for (source, expected) in cases {
            let formatted = Profile::LexerRoundtrip.pipeline()(source);

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
            let formatted =
                Profile::CstRoundtrip.pipeline()(source).map_err(|r| (r.line, r.column));

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
                "import a.B;\r\nclass A{B b;}\r\n",
                Ok("import a.B;\n\nclass A {\n  B b;\n}\n".to_owned()),
            ),
            ("class A { int \\u0061b; }\n", Err((1, 15))),
            ("class A {}\r", Ok("class A {}\n".to_owned())),
            ("class A {", Err((1, 10))),
            ("class A {\r\nvoid é;\r\n}\r\n", Err((2, 7))),
            ("class A {\r\n\r\n\r\n  int x = 1 }", Err((4, 13))),
        ];
        for (source, expected) in cases {
            let formatted = Profile::Gjf.pipeline()(source).map_err(|r| (r.line, r.column));

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
            let formatted = profile.pipeline()(source).map_err(|r| (r.line, r.column));

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

    #[test]
    fn when_padded_cell_with_each_kind_of_sequence_then_returns_its_canonical_form() {
        let sequences: [(&str, &[&str], &str); 5] = [
            ("a", &["a"], "a"),
            ("a", &["b", "b"], "b"),
            ("a", &["b", "c", "d", "d"], "d"),
            ("a", &["bbb", "cc", "dddd", "cc"], "cc"),
            (
                "a",
                &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"],
                "a",
            ),
        ];
        for (source, outputs, expected) in sequences {
            let calls = std::cell::Cell::new(0);
            let format = |_: &str| {
                let output = outputs[calls.get().min(outputs.len() - 1)];
                calls.set(calls.get() + 1);
                Ok(output.to_owned())
            };

            let canonical = padded_cell(format)(source);

            assert_eq!(canonical, Ok(expected.to_owned()), "{outputs:?}");
        }
    }

    #[test]
    fn when_padded_cell_with_a_rejecting_pass_then_returns_the_rejection() {
        let rejected = Located {
            line: 1,
            column: 1,
            message: "bad".to_owned(),
        };
        let format = |text: &str| match text {
            "a" => Ok("b".to_owned()),
            _ => Err(Located {
                line: 1,
                column: 1,
                message: "bad".to_owned(),
            }),
        };

        let canonical = padded_cell(format)("a");

        assert_eq!(canonical, Err(rejected));
    }
}
