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

pub fn parse(text: &str) -> Option<Positions> {
    let lines: Vec<(usize, &str)> = text
        .split('\n')
        .scan(0, |start, line| {
            let at = *start;
            *start += line.len() + 1;
            Some((at, line))
        })
        .collect();
    let blank = |line: &str| line.trim_start_matches([' ', '\t']).is_empty();
    let ported = !text.contains(['`', '|', '<', '\0'])
        && lines.iter().all(|(_, line)| {
            let content = line.trim_start_matches(' ');
            blank(line)
                || (line.len() - content.len() < 4
                    && !content.starts_with(|c: char| "\t#>-+*=_~<[".contains(c))
                    && !content.starts_with(|c: char| c.is_ascii_digit()))
        });
    let paragraphs = lines
        .split(|(_, line)| blank(line))
        .filter_map(|run| Some((run.first()?, run.last()?)))
        .flat_map(|((start, _), (last, line))| {
            [
                (*start, Token::from((Kind::ParagraphOpen, ""))),
                (last + line.len(), Token::from((Kind::ParagraphClose, ""))),
            ]
        })
        .map(|(position, token)| (position, vec![token]))
        .collect();
    ported.then_some(Positions(paragraphs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_with_paragraphs_then_returns_their_open_and_close_positions() {
        let cases: [(&str, &[(usize, Kind)]); 4] = [
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
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(
                expected
                    .iter()
                    .map(|&(position, kind)| (position, vec![Token::from((kind, ""))]))
                    .collect(),
            );
            assert_eq!(positions, Some(expected), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_construct_not_ported_yet_then_returns_none() {
        let cases = [
            "a `b`", "a | b", "a <ul> b", "a\0", "# a", "> a", "- a", "+ a", "* a", "1. a",
            "a\n===", "___", "~~~", "<p>", "[a]: b", "    a", "\ta", "a\n    b",
        ];
        for text in cases {
            let positions = parse(text);

            assert_eq!(positions, None, "{text:?}");
        }
    }
}
