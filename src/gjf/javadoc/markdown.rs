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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Document,
    List {
        marker: char,
    },
    Item {
        indent: usize,
        content: usize,
    },
    Quote,
    Paragraph {
        definition: Definition,
        first: bool,
    },
    Heading {
        inline: usize,
    },
    Fence {
        character: char,
        length: usize,
        indent: usize,
        closing: Option<usize>,
    },
    Table {
        lazy: bool,
    },
}

struct Open {
    block: Block,
    source: usize,
    start: Option<usize>,
    end: usize,
    children: usize,
    lines: Vec<(usize, usize)>,
    spans: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy)]
struct Cursor {
    index: usize,
    column: usize,
}

#[derive(Debug, Clone, Copy)]
enum Space {
    Blank(Cursor),
    Content { next: Cursor, indent: usize },
}

const COLUMN: fn(usize, char) -> usize = |column, c| match c {
    '\t' => column + 4 - column % 4,
    _ => column + 1,
};

const LEADING: fn(&str, Cursor) -> Cursor = |text, from| {
    let rest = &text[from.index..];
    let spaces = &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()];
    Cursor {
        index: from.index + spaces.len(),
        column: spaces.chars().fold(from.column, COLUMN),
    }
};

struct Parser<'a> {
    text: &'a str,
    open: Vec<Open>,
    events: Vec<(usize, Token)>,
    line: &'a str,
    offset: usize,
    cursor: Cursor,
}

enum Start {
    Blocks(Vec<Block>, usize),
    Heading(usize),
    Fence(Block, usize),
    Table,
}

impl<'a> Parser<'a> {
    fn space(&self) -> Space {
        let next = LEADING(self.line, self.cursor);
        match next.index == self.line.len() {
            true => Space::Blank(next),
            false => Space::Content {
                next,
                indent: next.column - self.cursor.column,
            },
        }
    }

    fn moved(&self, from: Cursor, reached: impl Fn(Cursor) -> bool) -> Cursor {
        let mut at = from;
        while let (false, Some(c)) = (reached(at), self.line[at.index..].chars().next()) {
            at = Cursor {
                index: at.index + c.len_utf8(),
                column: COLUMN(at.column, c),
            };
        }
        at
    }

    fn seek_column(&mut self, next: Cursor, column: usize) {
        let from = match column >= next.column {
            true => next,
            false => self.cursor,
        };
        let at = self.moved(from, |at| at.column >= column);
        self.cursor = match at.column > column {
            true => Cursor {
                index: at.index - 1,
                column,
            },
            false => at,
        };
    }

    fn close(&mut self, count: usize) {
        let closed = self.open.split_off(self.open.len() - count);
        for open in closed.into_iter().rev() {
            let end = open.end;
            let closing = match open.block {
                Block::Document => None,
                Block::List { .. } => Some(Kind::ListClose),
                Block::Item { .. } => Some(Kind::ListItemClose),
                Block::Quote => Some(Kind::MarkdownBlockQuoteClose),
                Block::Paragraph { first: true, .. } => None,
                Block::Paragraph { first: false, .. } => Some(Kind::ParagraphClose),
                Block::Heading { .. } => Some(Kind::HeaderClose),
                Block::Fence { .. } | Block::Table { .. } => None,
            };
            let start = open.start.unwrap_or(end);
            let opening = match open.block {
                Block::Paragraph { first: false, .. } => {
                    Some(Token::from((Kind::ParagraphOpen, "")))
                }
                Block::Table { .. } => {
                    Some(Token::from((Kind::MarkdownTable, &self.text[start..end])))
                }
                Block::Document
                | Block::List { .. }
                | Block::Item { .. }
                | Block::Quote
                | Block::Paragraph { first: true, .. }
                | Block::Heading { .. }
                | Block::Fence { .. } => None,
            };
            self.events.extend(opening.map(|token| (start, token)));
            let lines = match open.block {
                Block::Paragraph { .. } => open.lines.clone(),
                Block::Heading { inline } => vec![(inline, end)],
                Block::Document
                | Block::List { .. }
                | Block::Item { .. }
                | Block::Quote
                | Block::Fence { .. }
                | Block::Table { .. } => Vec::new(),
            };
            let text = self.text;
            let fenced = match open.block {
                Block::Fence {
                    character,
                    length,
                    indent,
                    closing,
                } => {
                    let at = open.start.unwrap_or(end) + indent;
                    let first = open.lines.first().copied().unwrap_or((at, at));
                    let rest = open.lines.get(1..).unwrap_or(&[]);
                    let info = text[first.0..first.1].trim_matches(|c: char| c <= ' ');
                    let mut chars = info.chars().peekable();
                    let unescaped: String = std::iter::from_fn(|| {
                        let c = chars.next()?;
                        Some(match (c, chars.peek()) {
                            ('\\', Some(next)) if ESCAPABLE.contains(*next) => {
                                chars.next().unwrap_or(c)
                            }
                            _ => c,
                        })
                    })
                    .collect();
                    let literal: String = rest
                        .iter()
                        .map(|&(from, to)| format!("{}\n", &text[from..to]))
                        .collect();
                    let fence = |count: usize| character.to_string().repeat(count);
                    vec![
                        (
                            at,
                            Token::from((Kind::MarkdownFencedCodeBlock, &text[at..end])),
                        ),
                        (
                            at,
                            Token {
                                kind: Kind::FenceStart,
                                value: fence(length) + &unescaped,
                            },
                        ),
                        (
                            at,
                            Token {
                                kind: Kind::FenceLiteral,
                                value: literal,
                            },
                        ),
                        (
                            at,
                            Token {
                                kind: Kind::FenceEnd,
                                value: fence(closing.unwrap_or(length)),
                            },
                        ),
                    ]
                }
                Block::Document
                | Block::List { .. }
                | Block::Item { .. }
                | Block::Quote
                | Block::Paragraph { .. }
                | Block::Heading { .. }
                | Block::Table { .. } => Vec::new(),
            };
            self.events.extend(fenced);
            let content = lines
                .iter()
                .map(|&(from, to)| &text[from..to])
                .collect::<Vec<_>>()
                .join("\n");
            let offsets: Vec<(usize, usize)> = lines
                .iter()
                .scan(0, |at, &(from, to)| {
                    let here = *at;
                    *at += to - from + 1;
                    Some((here, from))
                })
                .collect();
            self.events
                .extend(
                    inlines(&content, 0, Vec::new())
                        .into_iter()
                        .map(|(position, token)| {
                            let (line, source) = offsets
                                .iter()
                                .rev()
                                .find(|(line, _)| *line <= position)
                                .copied()
                                .unwrap_or((0, 0));
                            (source + position - line, token)
                        }),
                );
            self.events
                .extend(closing.map(|kind| (end, Token::from((kind, "")))));
        }
    }

    fn add_source_spans(&mut self) -> Result<(), Stop> {
        let (offset, index, length) = (self.offset, self.cursor.index, self.line.len());
        let text = self.text;
        let opened = self.open.iter_mut().skip(1).filter_map(|open| {
            let block = open.source.min(index);
            (length != block).then(|| {
                open.end = offset + length;
                match open.block {
                    Block::Paragraph { .. } => open.spans.push((offset + block, offset + length)),
                    Block::Document
                    | Block::List { .. }
                    | Block::Item { .. }
                    | Block::Quote
                    | Block::Heading { .. }
                    | Block::Fence { .. }
                    | Block::Table { .. } => {}
                }
                match open.start {
                    Some(_) => None,
                    None => {
                        open.start = Some(offset + block);
                        Some((open.block, offset + block))
                    }
                }
            })
        });
        let starts: Vec<(Block, usize)> = opened.flatten().collect();
        starts.into_iter().try_for_each(|(block, start)| {
            let token = match block {
                Block::Document
                | Block::Paragraph { .. }
                | Block::Fence { .. }
                | Block::Table { .. } => None,
                Block::Quote => {
                    let marker = text[start..].find('>').map(|offset| start + offset);
                    self.events.extend(marker.map(|at| {
                        let length = 1 + usize::from(text[at + 1..].starts_with(' '));
                        (
                            at,
                            Token::from((Kind::MarkdownBlockQuoteOpen, &text[at..at + length])),
                        )
                    }));
                    None
                }
                Block::List { .. } => Some(Token::from((Kind::ListOpen, ""))),
                Block::Heading { .. } => Some(Token::from((Kind::HeaderOpen, ""))),
                Block::Item { indent, .. } => {
                    let at = start + indent;
                    let rest = text.get(at..).unwrap_or("");
                    let digits =
                        rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                    let marker = match digits {
                        0 => usize::from(rest.starts_with(['-', '+', '*'])),
                        _ => (digits + 1) * usize::from(rest[digits..].starts_with(['.', ')'])),
                    };
                    let after = rest[marker..].chars().next();
                    let space = after
                        .filter(|c| WHITESPACE.contains(c))
                        .map_or(0, char::len_utf8);
                    let token = Token::from((Kind::ListItemOpen, &rest[..marker + space]));
                    match (marker, after.is_none() || space > 0) {
                        (0, _) | (_, false) => return Err(Stop::NotPorted),
                        (_, true) => self.events.push((at, token)),
                    }
                    None
                }
            };
            self.events.extend(token.map(|token| (start, token)));
            Ok(())
        })
    }

    fn add_line(&mut self) -> Result<(), Stop> {
        let content = &self.line[self.cursor.index..];
        let span = (
            self.offset + self.cursor.index,
            self.offset + self.line.len(),
        );
        match self.open.last_mut() {
            Some(Open {
                block:
                    Block::Paragraph {
                        definition: state, ..
                    },
                lines,
                ..
            }) => {
                *state = match definition(*state, content) {
                    Definition::Found => return Err(Stop::Unsupported),
                    next => next,
                };
                lines.push(span);
            }
            Some(Open {
                block: Block::Fence { .. },
                lines,
                ..
            }) => lines.push(span),
            Some(Open {
                block:
                    Block::Document
                    | Block::List { .. }
                    | Block::Item { .. }
                    | Block::Quote
                    | Block::Heading { .. }
                    | Block::Table { .. },
                ..
            })
            | None => {}
        }
        self.add_source_spans()
    }

    fn add_child(&mut self, block: Block, source: usize) {
        let contains = |parent: Block| match (parent, block) {
            (Block::Document | Block::Item { .. } | Block::Quote, _) => true,
            (Block::List { .. }, Block::Item { .. }) => true,
            (
                Block::List { .. }
                | Block::Paragraph { .. }
                | Block::Heading { .. }
                | Block::Fence { .. }
                | Block::Table { .. },
                _,
            ) => false,
        };
        while self.open.last().is_some_and(|open| !contains(open.block)) {
            self.close(1);
        }
        let block = match (
            self.open.last().map(|open| (open.block, open.children)),
            block,
        ) {
            (Some((Block::Item { .. } | Block::Quote, 0)), Block::Paragraph { definition, .. }) => {
                Block::Paragraph {
                    definition,
                    first: true,
                }
            }
            (_, block) => block,
        };
        self.open
            .iter_mut()
            .last()
            .into_iter()
            .for_each(|parent| parent.children += 1);
        self.open.push(Open {
            block,
            source,
            start: None,
            end: 0,
            children: 0,
            lines: Vec::new(),
            spans: Vec::new(),
        });
    }

    fn block_start(
        &self,
        matched: Block,
        next: Cursor,
        indent: usize,
    ) -> Result<Option<Start>, Stop> {
        let content = &self.line[next.index..];
        let run = |marker: char| content.len() - content.trim_start_matches(marker).len();
        let spaced = |rest: &str| rest.chars().next().is_none_or(|c| c == ' ' || c == '\t');
        let paragraph = matches!(matched, Block::Paragraph { .. });
        let active = self.open.last().map(|open| open.block);
        let hashes = run('#');
        let opening = content.len() - content.trim_start_matches(['`', '~']).len();
        let (ticks, tildes) = (
            content[..opening].matches('`').count(),
            content[..opening].matches('~').count(),
        );
        let fence = match (ticks >= 3 && tildes == 0, tildes >= 3 && ticks == 0) {
            (true, _) if !content[ticks..].contains('`') => Some(('`', ticks, &content[ticks..])),
            (_, true) => Some(('~', tildes, &content[tildes..])),
            (true, _) | (false, false) => None,
        };
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
        let header = match self.open.last() {
            Some(Open {
                block: Block::Paragraph { .. },
                lines,
                ..
            }) if paragraph => lines.last().map(|&(from, to)| &self.text[from..to]),
            Some(_) | None => None,
        };
        let table = header
            .filter(|header| header.contains('|'))
            .is_some_and(|header| {
                separator(&self.line[self.cursor.index..], 0, false, 0)
                    .is_some_and(|columns| columns > 0 && columns >= cells(header))
            });
        match indent >= 4 {
            _ if table => Ok(Some(Start::Table)),
            true => match active {
                Some(Block::Paragraph { .. }) => Ok(None),
                _ => Err(Stop::Unsupported),
            },
            false if content.starts_with('>') => {
                let spaced = content[1..].starts_with([' ', '\t']);
                Ok(Some(Start::Blocks(
                    vec![Block::Quote],
                    next.column + 1 + usize::from(spaced),
                )))
            }
            false if (1..=6).contains(&hashes) && spaced(&content[hashes..]) => {
                Ok(Some(Start::Heading(self.offset + next.index + hashes)))
            }
            false if paragraph && underline => Err(Stop::Unsupported),
            false if let Some((character, length, rest)) = fence => match rest.contains('&') {
                true => Err(Stop::NotPorted),
                false => Ok(Some(Start::Fence(
                    Block::Fence {
                        character,
                        length,
                        indent,
                        closing: None,
                    },
                    next.index + length,
                ))),
            },
            false if thematic => Err(Stop::Unsupported),
            false => self.list(matched, paragraph, next, indent),
        }
    }

    fn list(
        &self,
        matched: Block,
        paragraph: bool,
        next: Cursor,
        indent: usize,
    ) -> Result<Option<Start>, Stop> {
        let content = &self.line[next.index..];
        let digits = content.len()
            - content
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .len();
        let Some((marker, ordered)) = (match content.chars().next() {
            Some(c @ ('-' | '+' | '*')) => Some((c, None)),
            _ if (1..=9).contains(&digits) => content[digits..]
                .chars()
                .next()
                .filter(|c| ".)".contains(*c))
                .map(|c| (c, Some(&content[..digits]))),
            _ => None,
        }) else {
            return Ok(None);
        };
        let after = ordered.map_or(1, |number| number.len() + 1);
        let marked = Cursor {
            index: next.index + after,
            column: next.column + after,
        };
        let gap = LEADING(self.line, marked);
        let rest = &self.line[marked.index..];
        let valid = rest.chars().next().is_none_or(|c| c == ' ' || c == '\t');
        let empty = gap.index == self.line.len();
        let content_column = match empty || gap.column - marked.column > 4 {
            true => marked.column + 1,
            false => gap.column,
        };
        let interrupts =
            !paragraph || (!empty && ordered.is_none_or(|number| number.parse::<u64>() == Ok(1)));
        let item = Block::Item {
            indent,
            content: content_column - self.cursor.column,
        };
        let continues = matches!(matched, Block::List { marker: open } if open == marker);
        match (valid && interrupts, ordered, rest.starts_with('\t')) {
            (false, _, _) => Ok(None),
            (true, None, true) => Err(Stop::NotPorted),
            (true, _, _) => Ok(Some(match continues {
                true => Start::Blocks(vec![item], content_column),
                false => Start::Blocks(vec![Block::List { marker }, item], content_column),
            })),
        }
    }

    fn continued(&mut self, matches: usize) -> Matched {
        let Some(open) = self.open.get(matches) else {
            return Matched::Count(matches);
        };
        let (block, children) = (open.block, open.children);
        let space = self.space();
        let target = match (block, space) {
            (Block::Document | Block::List { .. }, _) => Some(Continue::Here),
            (Block::Quote, Space::Content { next, indent })
                if indent < 4 && self.line[next.index..].starts_with('>') =>
            {
                let spaced = self.line[next.index + 1..].starts_with([' ', '\t']);
                Some(Continue::Column(
                    next,
                    next.column + 1 + usize::from(spaced),
                ))
            }
            (Block::Quote, _) => None,
            (Block::Item { .. }, Space::Blank(_)) if children == 0 => None,
            (Block::Item { .. }, Space::Blank(next)) => Some(Continue::Index(next)),
            (Block::Item { content, .. }, Space::Content { next, indent }) if indent >= content => {
                Some(Continue::Column(next, self.cursor.column + content))
            }
            (Block::Item { .. }, Space::Content { .. }) => None,
            (Block::Paragraph { .. }, Space::Content { .. }) => Some(Continue::Here),
            (Block::Paragraph { .. }, Space::Blank(_)) | (Block::Heading { .. }, _) => None,
            (
                Block::Fence {
                    character,
                    length,
                    indent: fence,
                    ..
                },
                Space::Content { next, indent },
            ) if indent < 4 => {
                let rest = &self.line[next.index..];
                let fences = rest.len() - rest.trim_start_matches(character).len();
                let closes =
                    fences >= length && rest[fences..].trim_start_matches([' ', '\t']).is_empty();
                match closes {
                    true => Some(Continue::Finish(Block::Fence {
                        character,
                        length,
                        indent: fence,
                        closing: Some(fences),
                    })),
                    false => Some(Continue::Spaces(fence)),
                }
            }
            (Block::Fence { indent, .. }, _) => Some(Continue::Spaces(indent)),
            (Block::Table { .. }, Space::Content { next, .. }) => {
                let rest = &self.line[next.index..];
                match rest.find('|') {
                    Some(0) if rest[1..].trim_start_matches([' ', '\t']).is_empty() => {
                        Some(Continue::EndTable)
                    }
                    Some(_) => Some(Continue::Here),
                    None => None,
                }
            }
            (Block::Table { .. }, Space::Blank(_)) => None,
        };
        let Some(target) = target else {
            return Matched::Count(matches);
        };
        self.open[matches].source = self.cursor.index;
        match target {
            Continue::Here => {}
            Continue::Index(next) => self.cursor = next,
            Continue::Column(next, column) => self.seek_column(next, column),
            Continue::Spaces(indent) => {
                let start = self.cursor.index;
                self.cursor = self.moved(self.cursor, |at| {
                    at.index - start >= indent || !self.line[at.index..].starts_with(' ')
                });
            }
            Continue::EndTable => {
                self.open[matches].block = Block::Table { lazy: false };
                return Matched::Count(matches);
            }
            Continue::Finish(fence) => {
                self.open[matches].block = fence;
                return Matched::Finished(matches);
            }
        }
        self.continued(matches + 1)
    }

    fn starts(&mut self, matched: Block, opened: Opened) -> Result<Outcome, Stop> {
        let last = self.cursor.index;
        let space = self.space();
        let start = match (matched, space) {
            (Block::Fence { .. } | Block::Table { .. }, _) => {
                return Ok(Outcome {
                    matched,
                    opened,
                    last,
                    space,
                });
            }
            (_, Space::Blank(_)) => None,
            (_, Space::Content { next, indent }) => {
                let letter = self.line[next.index..].starts_with(char::is_alphabetic);
                match indent < 4 && letter {
                    true => None,
                    false => self.block_start(matched, next, indent)?,
                }
            }
        };
        let done = Outcome {
            matched,
            opened,
            last,
            space,
        };
        let next = match space {
            Space::Blank(next) | Space::Content { next, .. } => next,
        };
        let Some(start) = start else {
            self.cursor = next;
            return Ok(done);
        };
        let source = self.cursor.index;
        self.close(match opened {
            Opened::Nothing { unmatched } => unmatched,
            Opened::Blocks => 0,
        });
        let blocks = match start {
            Start::Blocks(blocks, column) => {
                self.seek_column(next, column);
                blocks
            }
            Start::Heading(inline) => {
                self.cursor = self.moved(next, |_| false);
                vec![Block::Heading { inline }]
            }
            Start::Fence(fence, index) => {
                self.cursor = self.moved(next, |at| at.index >= index);
                vec![fence]
            }
            Start::Table => {
                let paragraph = self.open.pop();
                let span = paragraph.and_then(|mut paragraph| {
                    let span = paragraph.spans.pop();
                    paragraph.lines.pop();
                    paragraph.end = paragraph.spans.last().map_or(0, |&(_, end)| end);
                    if !paragraph.lines.is_empty() {
                        self.open.push(paragraph);
                        self.close(1);
                    }
                    span
                });
                self.add_child(Block::Table { lazy: true }, source);
                self.open
                    .iter_mut()
                    .last()
                    .into_iter()
                    .zip(span)
                    .for_each(|(table, span)| table.start = Some(span.0));
                Vec::new()
            }
        };
        let matched = blocks.into_iter().fold(matched, |_, block| {
            self.add_child(block, source);
            block
        });
        match matched {
            Block::List { .. } | Block::Item { .. } | Block::Quote => {
                self.starts(matched, Opened::Blocks)
            }
            Block::Document
            | Block::Paragraph { .. }
            | Block::Heading { .. }
            | Block::Fence { .. }
            | Block::Table { .. } => Ok(Outcome {
                matched,
                opened: Opened::Blocks,
                last: self.cursor.index,
                space,
            }),
        }
    }

    fn parse_line(&mut self, line: &'a str, offset: usize) -> Result<(), Stop> {
        self.line = line;
        self.offset = offset;
        self.cursor = Cursor {
            index: 0,
            column: 0,
        };
        let matches = match self.continued(1) {
            Matched::Count(matches) => matches,
            Matched::Finished(fence) => {
                self.add_source_spans()?;
                self.close(self.open.len() - fence);
                return Ok(());
            }
        };
        let unmatched = self.open.len() - matches;
        let Outcome {
            matched,
            opened,
            last,
            space,
        } = self.starts(self.open[matches - 1].block, Opened::Nothing { unmatched })?;
        let active = self.open.last().map(|open| open.block);
        let unmatched = match opened {
            Opened::Nothing { unmatched } => unmatched,
            Opened::Blocks => 0,
        };
        match (opened, space, matched, active) {
            (
                Opened::Nothing { .. },
                Space::Content { .. },
                _,
                Some(Block::Paragraph { .. } | Block::Table { lazy: true }),
            ) => {
                self.open
                    .iter_mut()
                    .last()
                    .into_iter()
                    .for_each(|open| open.source = last);
                self.add_line()
            }
            (
                _,
                _,
                Block::Paragraph { .. }
                | Block::Heading { .. }
                | Block::Fence { .. }
                | Block::Table { .. },
                _,
            ) => {
                self.close(unmatched);
                self.add_line()
            }
            (_, Space::Content { .. }, _, _) => {
                self.close(unmatched);
                self.add_child(
                    Block::Paragraph {
                        definition: Definition::Start,
                        first: false,
                    },
                    last,
                );
                self.add_line()
            }
            (_, Space::Blank(_), _, _) => {
                self.close(unmatched);
                self.add_source_spans()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Opened {
    Nothing { unmatched: usize },
    Blocks,
}

struct Outcome {
    matched: Block,
    opened: Opened,
    last: usize,
    space: Space,
}

enum Continue {
    Here,
    Index(Cursor),
    Column(Cursor, usize),
    Spaces(usize),
    Finish(Block),
    EndTable,
}

enum Matched {
    Count(usize),
    Finished(usize),
}

fn cells(row: &str) -> usize {
    let trimmed = row.trim_start_matches([' ', '\t']);
    let body = match trimmed.strip_prefix('|') {
        Some(rest) => rest.trim_end_matches([' ', '\t']),
        None => trimmed,
    };
    let unescaped = body.replace("\\|", "x");
    let pieces: Vec<&str> = unescaped.split('|').collect();
    pieces.len() - 1 + usize::from(pieces.last().is_some_and(|last| !last.is_empty()))
}

fn separator(rest: &str, pipes: usize, valid: bool, columns: usize) -> Option<usize> {
    match rest.chars().next() {
        None => valid.then_some(columns),
        Some('|') => (pipes == 0)
            .then(|| separator(&rest[1..], pipes + 1, true, columns))
            .flatten(),
        Some('-' | ':') if pipes > 0 || columns == 0 => {
            let after = rest.strip_prefix(':').unwrap_or(rest);
            let dashes = after.len() - after.trim_start_matches('-').len();
            let tail = &after[dashes..];
            (dashes > 0)
                .then(|| {
                    separator(
                        tail.strip_prefix(':').unwrap_or(tail),
                        0,
                        valid,
                        columns + 1,
                    )
                })
                .flatten()
        }
        Some(' ' | '\t') => separator(&rest[1..], pipes, valid, columns),
        Some(_) => None,
    }
}

pub fn parse(text: &str) -> Option<Result<Positions, Unsupported>> {
    let mut parser = Parser {
        text,
        open: vec![Open {
            block: Block::Document,
            source: 0,
            start: None,
            end: 0,
            children: 0,
            lines: Vec::new(),
            spans: Vec::new(),
        }],
        events: Vec::new(),
        line: "",
        offset: 0,
        cursor: Cursor {
            index: 0,
            column: 0,
        },
    };
    let lines = text.split_inclusive('\n').scan(0, |start, line| {
        let at = *start;
        *start += line.len();
        Some((at, line.strip_suffix('\n').unwrap_or(line)))
    });
    let parsed = lines
        .map(|(offset, line)| parser.parse_line(line, offset))
        .collect::<Result<Vec<()>, Stop>>()
        .map(|_| {
            parser.close(parser.open.len());
            let closing = |token: &Token| {
                matches!(
                    token.kind,
                    Kind::ListClose
                        | Kind::ListItemClose
                        | Kind::ParagraphClose
                        | Kind::HeaderClose
                        | Kind::MarkdownCodeSpanEnd
                        | Kind::MarkdownBlockQuoteClose
                )
            };
            let (firsts, lasts): (Vec<_>, Vec<_>) = std::mem::take(&mut parser.events)
                .into_iter()
                .partition(|(_, token)| closing(token));
            Positions(firsts.into_iter().chain(lasts).fold(
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
        text.contains(['<', '\0', '\u{85}', '\u{2028}', '\u{2029}']),
        parsed,
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
            "a <ul> b",
            "`a \u{2028}b`",
            "[a]: <b>",
            "[a]: <b",
            "[a]: <b<c>",
            "- a\n  \t- b",
            "-\ta",
            "-  - a\n    \t- -x",
            "a\0",
            "~~~ a&amp;b",
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

    #[test]
    fn when_parse_with_lists_then_returns_their_list_and_item_positions() {
        let token = |kind: Kind, value: &str| Token::from((kind, value));
        let cases = [
            (
                "- a",
                vec![
                    (
                        0,
                        vec![token(Kind::ListOpen, ""), token(Kind::ListItemOpen, "- ")],
                    ),
                    (
                        3,
                        vec![token(Kind::ListItemClose, ""), token(Kind::ListClose, "")],
                    ),
                ],
            ),
            (
                "a\n 1) b\n\n    c",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (1, vec![token(Kind::ParagraphClose, "")]),
                    (2, vec![token(Kind::ListOpen, "")]),
                    (3, vec![token(Kind::ListItemOpen, "1) ")]),
                    (13, vec![token(Kind::ParagraphOpen, "")]),
                    (
                        14,
                        vec![
                            token(Kind::ParagraphClose, ""),
                            token(Kind::ListItemClose, ""),
                            token(Kind::ListClose, ""),
                        ],
                    ),
                ],
            ),
            (
                "-",
                vec![
                    (
                        0,
                        vec![token(Kind::ListOpen, ""), token(Kind::ListItemOpen, "-")],
                    ),
                    (
                        1,
                        vec![token(Kind::ListItemClose, ""), token(Kind::ListClose, "")],
                    ),
                ],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.into_iter().collect());
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_block_quotes_then_returns_their_open_and_close_positions() {
        let token = |kind: Kind, value: &str| Token::from((kind, value));
        let cases = [
            (
                "> a",
                vec![
                    (0, vec![token(Kind::MarkdownBlockQuoteOpen, "> ")]),
                    (3, vec![token(Kind::MarkdownBlockQuoteClose, "")]),
                ],
            ),
            (
                ">a",
                vec![
                    (0, vec![token(Kind::MarkdownBlockQuoteOpen, ">")]),
                    (2, vec![token(Kind::MarkdownBlockQuoteClose, "")]),
                ],
            ),
            (
                "  > a",
                vec![
                    (2, vec![token(Kind::MarkdownBlockQuoteOpen, "> ")]),
                    (5, vec![token(Kind::MarkdownBlockQuoteClose, "")]),
                ],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.into_iter().collect());
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_fenced_code_block_then_returns_its_parts_at_its_start() {
        let token = |kind: Kind, value: &str| Token::from((kind, value));
        let cases = [
            (
                "```a\\_b\nx\n````",
                vec![(
                    0,
                    vec![
                        token(Kind::MarkdownFencedCodeBlock, "```a\\_b\nx\n````"),
                        token(Kind::FenceStart, "```a_b"),
                        token(Kind::FenceLiteral, "x\n"),
                        token(Kind::FenceEnd, "````"),
                    ],
                )],
            ),
            (
                " ~~~\n  y",
                vec![(
                    1,
                    vec![
                        token(Kind::MarkdownFencedCodeBlock, "~~~\n  y"),
                        token(Kind::FenceStart, "~~~"),
                        token(Kind::FenceLiteral, " y\n"),
                        token(Kind::FenceEnd, "~~~"),
                    ],
                )],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.into_iter().collect());
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_table_then_returns_its_text_at_its_start() {
        let token = |kind: Kind, value: &str| Token::from((kind, value));
        let cases = [
            (
                "| a |\n|---|",
                vec![(0, vec![token(Kind::MarkdownTable, "| a |\n|---|")])],
            ),
            (
                "b\n| a |\n|---|",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (1, vec![token(Kind::ParagraphClose, "")]),
                    (2, vec![token(Kind::MarkdownTable, "| a |\n|---|")]),
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
