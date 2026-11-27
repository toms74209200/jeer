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

const SKIP: fn(&str) -> &str = |text| text.trim_start_matches(WHITESPACE);

const CLOSING: fn(char) -> char = |open| match open {
    '(' => ')',
    quote => quote,
};

const TITLE: fn(&str, char) -> Option<&str> = |text, close| {
    scan(text, &|c| match c {
        _ if c == close => Some(true),
        '(' if close == ')' => Some(false),
        _ => None,
    })
};

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
    let failed = (Definition::Paragraph, "");
    let (next, rest) = match (state, rest.is_empty()) {
        (_, true) | (Definition::Paragraph | Definition::Found, false) => return state,
        (Definition::Start, false) => SKIP(rest).strip_prefix('[').map_or(failed, |after| {
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
                Some(tail) if length <= 999 && !blank => (Definition::Destination, SKIP(tail)),
                None | Some(_) => failed,
            }
        }),
        (Definition::Destination, false) => {
            let scanned = balanced(SKIP(rest), 0);
            scanned.map_or(failed, |after| {
                let tail = SKIP(after);
                match (tail.is_empty(), tail.len() == after.len()) {
                    (true, _) => (Definition::Found, ""),
                    (false, true) => failed,
                    (false, false) => (Definition::StartTitle, tail),
                }
            })
        }
        (Definition::StartTitle, false) => {
            let text = SKIP(rest);
            match text.chars().next() {
                Some(c @ ('"' | '\'' | '(')) => (Definition::Title(CLOSING(c)), &text[1..]),
                None | Some(_) => (Definition::Start, text),
            }
        }
        (Definition::Title(delimiter), false) => {
            TITLE(rest, delimiter).map_or(failed, |after| match after.chars().next() {
                None => (Definition::Title(delimiter), ""),
                Some(c) => match SKIP(&after[c.len_utf8()..]).is_empty() {
                    true => (Definition::Found, ""),
                    false => failed,
                },
            })
        }
    };
    definition(next, rest)
}

fn closing(text: &str, from: usize, ticks: usize) -> Option<usize> {
    let start = from + text[from..].find('`')?;
    let run = text[start..].len() - text[start..].trim_start_matches('`').len();
    match run == ticks {
        true => Some(start),
        false => closing(text, start + run, ticks),
    }
}

#[derive(Debug, Clone, Copy)]
struct Bracket {
    image: bool,
    allowed: bool,
}

fn link(text: &str, from: usize) -> Option<usize> {
    let inner = SKIP(text[from..].strip_prefix('(')?);
    let destination = balanced(inner, 0)?;
    let spaced = SKIP(destination);
    let titled = match (spaced.len() < destination.len(), spaced.chars().next()) {
        (true, Some(open @ ('"' | '\'' | '('))) => {
            let close = CLOSING(open);
            TITLE(&spaced[1..], close)?.strip_prefix(close)?
        }
        _ => spaced,
    };
    let rest = SKIP(titled).strip_prefix(')')?;
    Some(text.len() - rest.len())
}

fn inlines(text: &str, from: usize, brackets: Vec<Bracket>) -> Vec<(usize, Token)> {
    let Some(start) = text[from..]
        .find(['`', '\\', '[', ']', '!'])
        .map(|offset| from + offset)
    else {
        return Vec::new();
    };
    let next = text[start + 1..].chars().next();
    let pushed = |image: bool| {
        brackets
            .iter()
            .copied()
            .chain([Bracket {
                image,
                allowed: true,
            }])
            .collect()
    };
    match text[start..].chars().next() {
        Some('\\') => {
            let escaped = next.filter(|c| ESCAPABLE.contains(*c));
            inlines(
                text,
                start + 1 + escaped.map_or(0, char::len_utf8),
                brackets,
            )
        }
        Some('[') => inlines(text, start + 1, pushed(false)),
        Some('!') if next == Some('[') => inlines(text, start + 2, pushed(true)),
        Some(']') => match brackets.split_last() {
            Some((opener, earlier)) if opener.allowed => match link(text, start + 1) {
                Some(end) => {
                    let disallowed = earlier.iter().map(|bracket| Bracket {
                        allowed: bracket.allowed && (opener.image || bracket.image),
                        ..*bracket
                    });
                    inlines(text, end, disallowed.collect())
                }
                None => inlines(text, start + 1, earlier.to_vec()),
            },
            Some((_, earlier)) => inlines(text, start + 1, earlier.to_vec()),
            None => inlines(text, start + 1, brackets),
        },
        Some('`') => {
            let run = text[start..].len() - text[start..].trim_start_matches('`').len();
            let ticks = &text[start..start + run];
            match closing(text, start + run, run) {
                Some(end) => [
                    (start, Token::from((Kind::MarkdownCodeSpanStart, ticks))),
                    (end, Token::from((Kind::MarkdownCodeSpanEnd, ticks))),
                ]
                .into_iter()
                .chain(inlines(text, end + run, brackets))
                .collect(),
                None => inlines(text, start + run, brackets),
            }
        }
        _ => inlines(text, start + 1, brackets),
    }
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
            (false, false) if run('~') >= 3 || run('`') >= 3 => Line::Stop(Stop::NotPorted),
            (false, false) if thematic => Line::Stop(Stop::Unsupported),
            (false, false) if bullet || ordered => Line::Stop(Stop::NotPorted),
            (false, false) => Line::Text,
        }
    };
    let block = |open: Kind, start: usize, inline: usize, close: Kind, end: usize| {
        std::iter::once((start, Token::from((open, ""))))
            .chain(inlines(&text[..end], inline, Vec::new()))
            .chain([(end, Token::from((close, "")))])
    };
    let close = |tokens: Vec<(usize, Token)>, paragraph: Option<(usize, usize, Definition)>| {
        let blocks = paragraph.map(|(start, end, _)| {
            block(Kind::ParagraphOpen, start, start, Kind::ParagraphClose, end)
        });
        tokens
            .into_iter()
            .chain(blocks.into_iter().flatten())
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
                    (Line::Heading, _) => {
                        let inline = end - content.trim_start_matches('#').len();
                        let heading =
                            block(Kind::HeaderOpen, start, inline, Kind::HeaderClose, end);
                        Ok((
                            close(tokens, paragraph)
                                .into_iter()
                                .chain(heading)
                                .collect(),
                            None,
                        ))
                    }
                    (Line::Text, None) => Ok((tokens, continued(start, Definition::Start)?)),
                    (Line::Stop(stop), _) => Err(stop),
                }
            },
        );
    let positions = blocks.map(|(tokens, paragraph)| {
        Positions(close(tokens, paragraph).into_iter().fold(
            HashMap::new(),
            |mut positions, (position, token)| {
                positions
                    .entry(position)
                    .or_insert_with(Vec::new)
                    .push(token);
                positions
            },
        ))
    });
    match (
        text.contains(['|', '<', '\0', '\u{85}', '\u{2028}', '\u{2029}']),
        positions,
    ) {
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
            "a | b",
            "a <ul> b",
            "`a \u{2028}b`",
            "```",
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

    #[test]
    fn when_parse_with_code_spans_then_returns_their_start_and_end_positions() {
        let open = |kind: Kind| Token::from((kind, ""));
        let tick = |kind: Kind, ticks: &str| Token::from((kind, ticks));
        let cases = [
            (
                "a `b` c",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (2, vec![tick(Kind::MarkdownCodeSpanStart, "`")]),
                    (4, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (7, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "`a`",
                vec![
                    (
                        0,
                        vec![
                            open(Kind::ParagraphOpen),
                            tick(Kind::MarkdownCodeSpanStart, "`"),
                        ],
                    ),
                    (2, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (3, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "`a`` b`",
                vec![
                    (
                        0,
                        vec![
                            open(Kind::ParagraphOpen),
                            tick(Kind::MarkdownCodeSpanStart, "`"),
                        ],
                    ),
                    (6, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (7, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "a ``b\nc`` d",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (2, vec![tick(Kind::MarkdownCodeSpanStart, "``")]),
                    (7, vec![tick(Kind::MarkdownCodeSpanEnd, "``")]),
                    (11, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "\\`a `b",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (6, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "\\\\`a`",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (2, vec![tick(Kind::MarkdownCodeSpanStart, "`")]),
                    (4, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (5, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "a\\b `c`",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (4, vec![tick(Kind::MarkdownCodeSpanStart, "`")]),
                    (6, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (7, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "a] !b `c`",
                vec![
                    (0, vec![open(Kind::ParagraphOpen)]),
                    (6, vec![tick(Kind::MarkdownCodeSpanStart, "`")]),
                    (8, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (9, vec![open(Kind::ParagraphClose)]),
                ],
            ),
            (
                "# `a`",
                vec![
                    (0, vec![open(Kind::HeaderOpen)]),
                    (2, vec![tick(Kind::MarkdownCodeSpanStart, "`")]),
                    (4, vec![tick(Kind::MarkdownCodeSpanEnd, "`")]),
                    (5, vec![open(Kind::HeaderClose)]),
                ],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.into_iter().collect());
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }
}
