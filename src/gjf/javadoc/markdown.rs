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
            (false, false) if !paragraph && content.starts_with('[') => Line::Stop(Stop::NotPorted),
            (false, false) => Line::Text,
        }
    };
    let close = |tokens: Vec<(usize, Token)>, paragraph: Option<(usize, usize)>| {
        tokens
            .into_iter()
            .chain(paragraph.into_iter().flat_map(|(start, end)| {
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
            (Vec::<(usize, Token)>::new(), None::<(usize, usize)>),
            |(tokens, paragraph), (start, line)| {
                let end = start + line.len();
                match (classify(line, paragraph.is_some()), paragraph) {
                    (Line::Blank, _) => Ok((close(tokens, paragraph), None)),
                    (Line::Indented | Line::Text, Some((open, _))) => {
                        Ok((tokens, Some((open, end))))
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
                    (Line::Text, None) => Ok((tokens, Some((start, end)))),
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
            "a\0",
            "> a",
            "- a",
            "+ a",
            "* a",
            "1. a",
            "1) a",
            "~~~",
            "[a]: b",
            "a\n\n[a]: b",
        ];
        for text in cases {
            let positions = parse(text);

            assert_eq!(positions, None, "{text:?}");
        }
    }
}
