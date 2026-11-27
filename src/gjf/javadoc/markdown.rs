use super::lexer::{Kind, Token};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub struct Positions(HashMap<usize, Vec<Token>>);

impl std::ops::Index<usize> for Positions {
    type Output = [Token];

    fn index(&self, position: usize) -> &[Token] {
        self.0.get(&position).map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Unsupported;

enum Stop {
    NotPorted,
    Unsupported,
}

enum Line {
    Blank,
    Indented,
    Heading,
    Underline,
    Text,
    Stop(Stop),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Definition {
    Start,
    Label { length: usize, blank: bool },
    Destination,
    StartTitle,
    Title(char),
    Paragraph,
    Found,
}

const WHITESPACE: [char; 6] = [' ', '\t', '\n', '\u{b}', '\u{c}', '\r'];

const ESCAPABLE: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

fn scan<'a>(text: &'a str, stop: &dyn Fn(char) -> Option<bool>) -> Option<&'a str> {
    match text.chars().next() {
        None => Some(text),
        Some('\\') => scan(
            text[1..]
                .strip_prefix(|c| ESCAPABLE.contains(c))
                .unwrap_or(&text[1..]),
            stop,
        ),
        Some(c) => match stop(c) {
            Some(true) => Some(text),
            Some(false) => None,
            None => scan(&text[c.len_utf8()..], stop),
        },
    }
}

fn balanced(text: &str, parens: usize) -> Option<&str> {
    match text.chars().next() {
        None => Some(text),
        Some('\\') => balanced(
            text[1..]
                .strip_prefix(|c| ESCAPABLE.contains(c))
                .unwrap_or(&text[1..]),
            parens,
        ),
        Some('(') => (parens < 32)
            .then(|| balanced(&text[1..], parens + 1))
            .flatten(),
        Some(')') => match parens {
            0 => Some(text),
            _ => balanced(&text[1..], parens - 1),
        },
        Some(c) if c == ' ' || c.is_control() => Some(text),
        Some(c) => balanced(&text[c.len_utf8()..], parens),
    }
}

fn definition(state: Definition, rest: &str) -> Definition {
    let skip: fn(&str) -> &str = |text| text.trim_start_matches(WHITESPACE);
    let failed = (Definition::Paragraph, "");
    let (next, rest) = match (state, rest.is_empty()) {
        (_, true) | (Definition::Paragraph | Definition::Found, false) => return state,
        (Definition::Start, false) => skip(rest).strip_prefix('[').map_or(failed, |after| {
            let length = usize::from(after.is_empty());
            (
                Definition::Label {
                    length,
                    blank: true,
                },
                after,
            )
        }),
        (Definition::Label { length, blank }, false) => scan(rest, &|c| match c {
            ']' => Some(true),
            '[' => Some(false),
            _ => None,
        })
        .map_or(failed, |after| {
            let content = &rest[..rest.len() - after.len()];
            let length = length + content.encode_utf16().count();
            let blank = blank && content.chars().all(|c| c <= ' ');
            match after.strip_prefix("]:") {
                None if after.is_empty() => (
                    Definition::Label {
                        length: length + 1,
                        blank,
                    },
                    "",
                ),
                Some(tail) if length <= 999 && !blank => (Definition::Destination, skip(tail)),
                None | Some(_) => failed,
            }
        }),
        (Definition::Destination, false) => {
            let scanned = balanced(skip(rest), 0);
            scanned.map_or(failed, |after| {
                let tail = skip(after);
                match (tail.is_empty(), tail.len() == after.len()) {
                    (true, _) => (Definition::Found, ""),
                    (false, true) => failed,
                    (false, false) => (Definition::StartTitle, tail),
                }
            })
        }
        (Definition::StartTitle, false) => {
            let text = skip(rest);
            match text.chars().next() {
                Some(c @ ('"' | '\'')) => (Definition::Title(c), &text[1..]),
                Some('(') => (Definition::Title(')'), &text[1..]),
                None | Some(_) => (Definition::Start, text),
            }
        }
        (Definition::Title(delimiter), false) => scan(rest, &|c| match c {
            _ if c == delimiter => Some(true),
            '(' if delimiter == ')' => Some(false),
            _ => None,
        })
        .map_or(failed, |after| match after.chars().next() {
            None => (Definition::Title(delimiter), ""),
            Some(c) => match skip(&after[c.len_utf8()..]).is_empty() {
                true => (Definition::Found, ""),
                false => failed,
            },
        }),
    };
    definition(next, rest)
}

pub fn parse(text: &str) -> Option<Result<Positions, Unsupported>> {
    let classify = |line: &str, paragraph: bool| {
        let content = line.trim_start_matches([' ', '\t']);
        let columns = line[..line.len() - content.len()]
            .chars()
            .fold(0, |column, c| match c {
                '\t' => column + 4 - column % 4,
                _ => column + 1,
            });
        let spaced = |rest: &str| rest.chars().next().is_none_or(|c| c == ' ' || c == '\t');
        let run = |marker: char| content.len() - content.trim_start_matches(marker).len();
        let hashes = run('#');
        let underline = content
            .chars()
            .next()
            .filter(|marker| "=-".contains(*marker))
            .is_some_and(|marker| {
                content
                    .trim_start_matches(marker)
                    .trim_start_matches([' ', '\t'])
                    .is_empty()
            });
        let markers = |marker: char| content.matches(marker).count();
        let thematic = content.chars().all(|c| "-_* \t".contains(c))
            && [('-', '_', '*'), ('_', '-', '*'), ('*', '-', '_')]
                .iter()
                .any(|&(kept, a, b)| markers(kept) >= 3 && markers(a) == 0 && markers(b) == 0);
        let digits = content.len()
            - content
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .len();
        let bullet = content.starts_with(['-', '+', '*']) && spaced(&content[1..]);
        let ordered = (1..=9).contains(&digits)
            && content[digits..].starts_with(['.', ')'])
            && spaced(&content[digits + 1..]);
        match (content.is_empty(), columns >= 4) {
            (true, _) => Line::Blank,
            (false, true) => Line::Indented,
            (false, false) if content.starts_with('>') => Line::Stop(Stop::NotPorted),
            (false, false) if (1..=6).contains(&hashes) && spaced(&content[hashes..]) => {
                Line::Heading
            }
            (false, false) if paragraph && underline => Line::Underline,
            (false, false) if run('~') >= 3 => Line::Stop(Stop::NotPorted),
            (false, false) if thematic => Line::Stop(Stop::Unsupported),
            (false, false) if bullet || ordered => Line::Stop(Stop::NotPorted),
            (false, false) => Line::Text,
        }
    };
    let close = |tokens: Vec<(usize, Token)>, paragraph: Option<(usize, usize, Definition)>| {
        tokens
            .into_iter()
            .chain(paragraph.into_iter().flat_map(|(start, end, _)| {
                [
                    (start, Token::from((Kind::ParagraphOpen, ""))),
                    (end, Token::from((Kind::ParagraphClose, ""))),
                ]
            }))
            .collect::<Vec<_>>()
    };
    let blocks = text
        .split('\n')
        .scan(0, |start, line| {
            let at = *start;
            *start += line.len() + 1;
            Some((at, line))
        })
        .try_fold(
            (
                Vec::<(usize, Token)>::new(),
                None::<(usize, usize, Definition)>,
            ),
            |(tokens, paragraph), (start, line)| {
                let end = start + line.len();
                let content = line.trim_start_matches([' ', '\t']);
                let continued = |open: usize, state: Definition| match definition(state, content) {
                    Definition::Found => Err(Stop::Unsupported),
                    state => Ok(Some((open, end, state))),
                };
                match (classify(line, paragraph.is_some()), paragraph) {
                    (Line::Blank, _) => Ok((close(tokens, paragraph), None)),
                    (Line::Indented | Line::Text, Some((open, _, state))) => {
                        Ok((tokens, continued(open, state)?))
                    }
                    (Line::Indented, None) | (Line::Underline, _) => Err(Stop::Unsupported),
                    (Line::Heading, _) => Ok((
                        close(tokens, paragraph)
                            .into_iter()
                            .chain([
                                (start, Token::from((Kind::HeaderOpen, ""))),
                                (end, Token::from((Kind::HeaderClose, ""))),
                            ])
                            .collect(),
                        None,
                    )),
                    (Line::Text, None) => Ok((tokens, continued(start, Definition::Start)?)),
                    (Line::Stop(stop), _) => Err(stop),
                }
            },
        );
    let positions = blocks.map(|(tokens, paragraph)| {
        Positions(
            close(tokens, paragraph)
                .into_iter()
                .map(|(position, token)| (position, vec![token]))
                .collect(),
        )
    });
    match (text.contains(['`', '|', '<', '\0']), positions) {
        (true, _) | (false, Err(Stop::NotPorted)) => None,
        (false, Ok(positions)) => Some(Ok(positions)),
        (false, Err(Stop::Unsupported)) => Some(Err(Unsupported)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_with_paragraphs_and_headings_then_returns_their_positions() {
        let cases: [(&str, &[(usize, Kind)]); 13] = [
            (
                "a b\nc",
                &[(0, Kind::ParagraphOpen), (5, Kind::ParagraphClose)],
            ),
            (
                "a\n\n\n  b  ",
                &[
                    (0, Kind::ParagraphOpen),
                    (1, Kind::ParagraphClose),
                    (4, Kind::ParagraphOpen),
                    (9, Kind::ParagraphClose),
                ],
            ),
            ("", &[]),
            ("\n \n", &[]),
            (
                "a\n# h\nb",
                &[
                    (0, Kind::ParagraphOpen),
                    (1, Kind::ParagraphClose),
                    (2, Kind::HeaderOpen),
                    (5, Kind::HeaderClose),
                    (6, Kind::ParagraphOpen),
                    (7, Kind::ParagraphClose),
                ],
            ),
            ("  #", &[(0, Kind::HeaderOpen), (3, Kind::HeaderClose)]),
            ("#a", &[(0, Kind::ParagraphOpen), (2, Kind::ParagraphClose)]),
            (
                "####### a",
                &[(0, Kind::ParagraphOpen), (9, Kind::ParagraphClose)],
            ),
            (
                "a\n    b",
                &[(0, Kind::ParagraphOpen), (7, Kind::ParagraphClose)],
            ),
            (
                "===",
                &[(0, Kind::ParagraphOpen), (3, Kind::ParagraphClose)],
            ),
            (
                "-a\n1.a",
                &[(0, Kind::ParagraphOpen), (6, Kind::ParagraphClose)],
            ),
            (
                "a\n[b]: c",
                &[(0, Kind::ParagraphOpen), (8, Kind::ParagraphClose)],
            ),
            (
                "~~ a",
                &[(0, Kind::ParagraphOpen), (4, Kind::ParagraphClose)],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(
                expected
                    .iter()
                    .map(|&(position, kind)| (position, vec![Token::from((kind, ""))]))
                    .collect(),
            );
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_block_google_java_format_does_not_support_then_returns_unsupported() {
        let cases = [
            "    a",
            " \ta",
            "a\n\n    b",
            "# h\n    b",
            "a\n===",
            "a\n-",
            "***",
            "- - -",
            "___",
        ];
        for text in cases {
            let positions = parse(text);

            assert_eq!(positions, Some(Err(Unsupported)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_construct_not_ported_yet_then_returns_none() {
        let cases = [
            "a `b`",
            "a | b",
            "a <ul> b",
            "[a]: <b>",
            "[a]: <b",
            "[a]: <b<c>",
            "a\0",
            "> a",
            "- a",
            "+ a",
            "* a",
            "1. a",
            "1) a",
            "~~~",
        ];
        for text in cases {
            let positions = parse(text);

            assert_eq!(positions, None, "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_link_reference_definition_then_returns_unsupported() {
        let deepest = format!("[a]: x{}{}", "(".repeat(32), ")".repeat(32));
        let cases = [
            "[a]: b",
            "[a]:\nb",
            "[a]: b \"t\"",
            "[a]: b\n\"t\"",
            "[a\nb]: c",
            "[a]: b 't\nu'",
            "[a]: (b)",
            "[a]: b\"t\"",
            "[a\\]]: b",
            "[a]: b(c",
            "[a]: b\\(c",
            deepest.as_str(),
            "a\n\n[a]: b",
        ];
        for text in cases {
            let positions = parse(text);

            assert_eq!(positions, Some(Err(Unsupported)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_brackets_that_define_nothing_then_returns_a_paragraph() {
        let long = format!("[{}]: b", "a".repeat(1000));
        let deeper = format!("[a]: x{}{}", "(".repeat(33), ")".repeat(33));
        let cases = [
            "[a]",
            "[a] b",
            "[a]:",
            "[ ]: b",
            "[]: b",
            "[a]: b c",
            "[a]: b \"t\" x",
            "[a[b]: c",
            "[a] : b",
            "[a]: b (t(u))",
            "Text first\n[a]: b",
            "[a]: b)c",
            "[a]: b\tc",
            long.as_str(),
            deeper.as_str(),
        ];
        for text in cases {
            let positions = parse(text);

            let expected = Positions(
                [
                    (0, vec![Token::from((Kind::ParagraphOpen, ""))]),
                    (text.len(), vec![Token::from((Kind::ParagraphClose, ""))]),
                ]
                .into_iter()
                .collect(),
            );
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }
}
