use super::entity_table;
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

const DESTINATION: fn(&str) -> Option<&str> = |text| match text.strip_prefix('<') {
    Some(inner) => scan(inner, &|c| match c {
        '>' => Some(true),
        '\n' | '<' => Some(false),
        _ => None,
    })
    .and_then(|rest| rest.strip_prefix('>')),
    None => balanced(text, 0),
};

const UNQUOTED_IN_BLOCK: fn(char) -> bool = |c| c > ' ' && !"\"'=<>`".contains(c);

const UNQUOTED_INLINE: fn(char) -> bool = |c| !WHITESPACE.contains(&c) && !"\"'=<>`".contains(c);

fn attributes(text: &str, unquoted: fn(char) -> bool) -> Option<&str> {
    let spaced = SKIP(text);
    let name = spaced
        .strip_prefix(|c: char| c.is_ascii_alphabetic() || "_:".contains(c))
        .filter(|_| spaced.len() < text.len());
    let Some(name) = name else {
        return spaced.strip_prefix('/').unwrap_or(spaced).strip_prefix('>');
    };
    let named = name.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || "_.:-".contains(c));
    let Some(value) = SKIP(named).strip_prefix('=') else {
        return attributes(named, unquoted);
    };
    let value = SKIP(value);
    let rest = match value.chars().next() {
        Some(quote @ ('"' | '\'')) => value[1..].find(quote).map(|end| &value[end + 2..]),
        _ => Some(value.trim_start_matches(unquoted)).filter(|rest| rest.len() < value.len()),
    }?;
    attributes(rest, unquoted)
}

const TAG_NAME: fn(&str) -> Option<&str> = |rest| {
    rest.strip_prefix(|c: char| c.is_ascii_alphabetic())
        .map(|after| after.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '-'))
};

fn tag(text: &str, unquoted: fn(char) -> bool) -> Option<&str> {
    let inner = text.strip_prefix('<')?;
    match inner.strip_prefix('/') {
        Some(closing) => SKIP(TAG_NAME(closing)?).strip_prefix('>'),
        None => attributes(TAG_NAME(inner)?, unquoted),
    }
}

const AUTOLINK: fn(&str) -> Option<&str> = |text| {
    let (content, rest) = text.split_once('>')?;
    let uri = content.split_once(':').is_some_and(|(scheme, rest)| {
        (2..=32).contains(&scheme.len())
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".+-".contains(c))
            && !rest.contains(|c: char| c <= ' ' || c == '<' || c == '>')
    });
    let email = content.split_once('@').is_some_and(|(local, domain)| {
        local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c))
            && domain.split('.').all(|label| {
                (1..=63).contains(&label.len())
                    && label.starts_with(|c: char| c.is_ascii_alphanumeric())
                    && label.ends_with(|c: char| c.is_ascii_alphanumeric())
                    && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    });
    (uri || email).then_some(rest)
};

fn inline_html(text: &str) -> Option<&str> {
    let inner = &text[1..];
    match inner.chars().next()? {
        '?' => {
            let body = &inner[1..];
            let end = body.find("?>")?;
            (!body.starts_with('?') && !body[..=end].contains("??")).then(|| &body[end + 2..])
        }
        '!' => {
            let bang = &inner[1..];
            match bang.chars().next()? {
                '-' => bang.strip_prefix("--").and_then(|body| {
                    body.strip_prefix('>')
                        .or_else(|| body.strip_prefix("->"))
                        .or_else(|| body.find("-->").map(|end| &body[end + 3..]))
                }),
                '[' => bang
                    .strip_prefix("[CDATA[")
                    .and_then(|body| body.find("]]>").map(|end| &body[end + 3..])),
                letter if letter.is_ascii_alphabetic() => {
                    let letters = bang.trim_start_matches(|c: char| c.is_ascii_alphabetic());
                    let spaced = SKIP(letters);
                    (spaced.len() < letters.len())
                        .then(|| spaced.find('>').map(|end| &spaced[end + 1..]))
                        .flatten()
                }
                _ => None,
            }
        }
        _ => tag(text, UNQUOTED_INLINE),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HtmlStart {
    Interrupting(Ending),
    NotInterrupting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    BlankLine,
    Closer(&'static [&'static str]),
    Finished,
}

const RAW_TEXT: [&str; 4] = ["script", "pre", "style", "textarea"];

const BLOCK_NAMES: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

fn html_block(content: &str) -> Option<HtmlStart> {
    let lower = content.to_ascii_lowercase();
    let inner = lower.strip_prefix('<')?;
    let spaced = |rest: &str| rest.is_empty() || rest.starts_with(WHITESPACE);
    let named = |names: &[&str], rest: &str, ends: &dyn Fn(&str) -> bool| {
        names
            .iter()
            .any(|name| rest.strip_prefix(name).is_some_and(ends))
    };
    let starts = [
        (
            named(&RAW_TEXT, inner, &|rest| {
                spaced(rest) || rest.starts_with('>')
            }),
            HtmlStart::Interrupting(Ending::Closer(&[
                "</script>",
                "</pre>",
                "</style>",
                "</textarea>",
            ])),
        ),
        (
            inner.starts_with("!--"),
            HtmlStart::Interrupting(Ending::Closer(&["-->"])),
        ),
        (
            inner.starts_with('?'),
            HtmlStart::Interrupting(Ending::Closer(&["?>"])),
        ),
        (
            content[1..]
                .strip_prefix('!')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase())),
            HtmlStart::Interrupting(Ending::Closer(&[">"])),
        ),
        (
            content.starts_with("<![CDATA["),
            HtmlStart::Interrupting(Ending::Closer(&["]]>"])),
        ),
        (
            named(
                &BLOCK_NAMES,
                inner.strip_prefix('/').unwrap_or(inner),
                &|rest| spaced(rest) || rest.strip_prefix('/').unwrap_or(rest).starts_with('>'),
            ),
            HtmlStart::Interrupting(Ending::BlankLine),
        ),
        (
            tag(content, UNQUOTED_IN_BLOCK).is_some_and(|rest| SKIP(rest).is_empty()),
            HtmlStart::NotInterrupting,
        ),
    ];
    starts
        .into_iter()
        .find_map(|(starts, ending)| starts.then_some(ending))
}

const UNITS: fn(u32) -> Vec<u16> = |code| match (code, char::from_u32(code)) {
    (0, _) => vec![0xFFFD],
    (_, Some(decoded)) => decoded.encode_utf16(&mut [0; 2]).to_vec(),
    (_, None) => u16::try_from(code).map_or(vec![0xFFFD], |unit| vec![unit]),
};

fn unescape(text: &str) -> Vec<u16> {
    let Some(at) = text.find(['\\', '&']) else {
        return text.encode_utf16().collect();
    };
    let rest = &text[at..];
    let escaped = rest
        .strip_prefix('\\')
        .and_then(|tail| tail.chars().next())
        .filter(|c| ESCAPABLE.contains(*c))
        .map(|c| (c.encode_utf16(&mut [0; 2]).to_vec(), 1 + c.len_utf8()));
    let entity = rest.strip_prefix('&').and_then(|tail| {
        let Some(number) = tail.strip_prefix('#') else {
            let run = tail.len()
                - tail
                    .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
                    .len();
            let found = entity_table::NAMED_CHARACTER_REFERENCES
                .binary_search_by(|(name, _)| (*name).cmp(&tail[..run]))
                .ok()
                .filter(|_| tail[run..].starts_with(';'))?;
            let value = entity_table::NAMED_CHARACTER_REFERENCES[found].1;
            return Some((value.encode_utf16().collect(), 1 + run + 1));
        };
        let (digits, radix, most, prefix) = match number.strip_prefix(['x', 'X']) {
            Some(hex) => (hex, 16, 6, 2),
            None => (number, 10, 7, 1),
        };
        let run = digits.len() - digits.trim_start_matches(|c: char| c.is_digit(radix)).len();
        (run <= most && digits[run..].starts_with(';'))
            .then_some(())
            .and_then(|()| u32::from_str_radix(&digits[..run], radix).ok())
            .map(|code| (UNITS(code), 1 + prefix + run + 1))
    });
    let (decoded, length) = escaped
        .or(entity)
        .unwrap_or_else(|| (vec![rest.as_bytes()[0].into()], 1));
    text[..at]
        .encode_utf16()
        .chain(decoded)
        .chain(unescape(&text[at + length..]))
        .collect()
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
            let scanned = DESTINATION(SKIP(rest));
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
    let destination = DESTINATION(inner)?;
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
        .find(['`', '\\', '[', ']', '!', '<'])
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
        Some('<') => {
            let rest = &text[start..];
            let parsed = AUTOLINK(&rest[1..]).or_else(|| inline_html(rest));
            inlines(
                text,
                parsed.map_or(start + 1, |after| text.len() - after.len()),
                brackets,
            )
        }
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
    Html(Ending),
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

const LAZY: fn(Option<Block>) -> bool = |active| {
    matches!(
        active,
        Some(Block::Paragraph { .. } | Block::Table { lazy: true })
    )
};

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

const ORIGINAL: fn(&[usize], usize) -> usize =
    |replaced, position| position - 2 * replaced.partition_point(|&at| at < position);

struct Parser<'a> {
    text: &'a str,
    input: &'a str,
    replaced: &'a [usize],
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
    Html(Ending),
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
                Block::Fence { .. } | Block::Table { .. } | Block::Html(_) => None,
            };
            let start = open.start.unwrap_or(end);
            let opening = match open.block {
                Block::Paragraph { first: false, .. } => {
                    Some(Token::from((Kind::ParagraphOpen, "")))
                }
                Block::Table { .. } => Some(Token::from((
                    Kind::MarkdownTable,
                    &self.input[ORIGINAL(self.replaced, start)..ORIGINAL(self.replaced, end)],
                ))),
                Block::Document
                | Block::List { .. }
                | Block::Item { .. }
                | Block::Quote
                | Block::Paragraph { first: true, .. }
                | Block::Heading { .. }
                | Block::Fence { .. }
                | Block::Html(_) => None,
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
                | Block::Table { .. }
                | Block::Html(_) => Vec::new(),
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
                    let unescaped = String::from_utf16_lossy(&unescape(info));
                    let literal: String = rest
                        .iter()
                        .map(|&(from, to)| format!("{}\n", &text[from..to]))
                        .collect();
                    let fence = |count: usize| character.to_string().repeat(count);
                    vec![
                        (
                            at,
                            Token::from((
                                Kind::MarkdownFencedCodeBlock,
                                &self.input
                                    [ORIGINAL(self.replaced, at)..ORIGINAL(self.replaced, end)],
                            )),
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
                | Block::Table { .. }
                | Block::Html(_) => Vec::new(),
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
                    | Block::Table { .. }
                    | Block::Html(_) => {}
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
                | Block::Table { .. }
                | Block::Html(_) => None,
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
                block: Block::Html(ending),
                ..
            }) => {
                let lower = content.to_ascii_lowercase();
                *ending = match *ending {
                    Ending::Closer(closers)
                        if closers.iter().any(|closer| lower.contains(closer)) =>
                    {
                        Ending::Finished
                    }
                    Ending::BlankLine | Ending::Closer(_) | Ending::Finished => *ending,
                };
            }
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
                | Block::Table { .. }
                | Block::Html(_),
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
        let html = match (html_block(content), LAZY(active)) {
            (Some(HtmlStart::Interrupting(ending)), _) => Some(ending),
            (Some(HtmlStart::NotInterrupting), false) => Some(Ending::BlankLine),
            (Some(HtmlStart::NotInterrupting), true) | (None, _) => None,
        };
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
            false if let Some((character, length, info)) = fence => {
                let later = std::iter::successors(Some(Some(info.to_owned())), |text| {
                    let text = text.as_ref()?;
                    let next = String::from_utf16(&unescape(text)).ok();
                    (next.as_ref() != Some(text)).then_some(next)
                })
                .any(|text| text.is_none_or(|text| character == '`' && text.contains('`')));
                match later {
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
                }
            }
            false if let Some(ending) = html => Ok(Some(Start::Html(ending))),
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
            (Block::Html(Ending::Finished), _)
            | (Block::Html(Ending::BlankLine), Space::Blank(_)) => None,
            (Block::Html(_), _) => Some(Continue::Here),
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
            (Block::Fence { .. } | Block::Table { .. } | Block::Html(_), _) => {
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
            Start::Html(ending) => vec![Block::Html(ending)],
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
            | Block::Table { .. }
            | Block::Html(_) => Ok(Outcome {
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
            (Opened::Nothing { .. }, Space::Content { .. }, _, _) if LAZY(active) => {
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
                | Block::Table { .. }
                | Block::Html(_),
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
    let prepared = text.replace('\0', "\u{FFFD}");
    let replaced: Vec<usize> = text
        .match_indices('\0')
        .enumerate()
        .map(|(count, (at, _))| at + 2 * count)
        .collect();
    let mut parser = Parser {
        text: &prepared,
        input: text,
        replaced: &replaced,
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
    let lines = prepared.split_inclusive('\n').scan(0, |start, line| {
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
                        .entry(ORIGINAL(&replaced, position))
                        .or_insert_with(Vec::new)
                        .push(token);
                    positions
                },
            ))
        });
    match (text.contains(['\u{85}', '\u{2028}', '\u{2029}']), parsed) {
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
            "`a \u{2028}b`",
            "```a&#xDE00;b",
            "```a&#xD83D;&amp;#xDE00;b",
            "```a&amp;#xDE00;b",
            "```a&amp;amp;#xDE00;b",
            "```a&amp;#xD83D;&amp;amp;#xDE00;b",
            "```a &#96;a &#x2028;b&#96;",
            "- a\n  \t- b",
            "-\ta",
            "-  - a\n    \t- -x",
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
            "[a]: <b c>",
            "[a]: <b\\>c>",
            "[a]: <>",
            "[a]: <b> \"t\"",
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
            "[a]: <b",
            "[a]: <b<c>",
            "[a]: <b\nc>",
            "[a]: <b>c",
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
                "~~~a&#96;b",
                vec![(
                    0,
                    vec![
                        token(Kind::MarkdownFencedCodeBlock, "~~~a&#96;b"),
                        token(Kind::FenceStart, "~~~a`b"),
                        token(Kind::FenceLiteral, ""),
                        token(Kind::FenceEnd, "~~~"),
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

    #[test]
    fn when_parse_with_an_html_block_then_returns_only_the_paragraphs_around_it() {
        let quoted = [
            (0, Kind::MarkdownBlockQuoteOpen, "> "),
            (7, Kind::MarkdownBlockQuoteClose, ""),
            (8, Kind::ParagraphOpen, ""),
            (9, Kind::ParagraphClose, ""),
        ];
        let lazy = [
            (0, Kind::MarkdownBlockQuoteOpen, "> "),
            (7, Kind::MarkdownBlockQuoteClose, ""),
        ];
        let paragraphs = |spans: &[(usize, usize)]| -> Vec<(usize, Kind, &str)> {
            spans
                .iter()
                .flat_map(|&(open, close)| {
                    [
                        (open, Kind::ParagraphOpen, ""),
                        (close, Kind::ParagraphClose, ""),
                    ]
                })
                .collect()
        };
        let cases = [
            ("a\n<div>\nb", paragraphs(&[(0, 1)])),
            ("<div>\n\nb", paragraphs(&[(7, 8)])),
            ("<!-- a -->\nb", paragraphs(&[(11, 12)])),
            ("<!--\n\n-->\nb", paragraphs(&[(10, 11)])),
            ("<?\n?>\nb", paragraphs(&[(6, 7)])),
            ("<!A\n>\nb", paragraphs(&[(6, 7)])),
            ("<![CDATA[\n]]>\nb", paragraphs(&[(14, 15)])),
            ("<PRE>\n</Pre>\nb", paragraphs(&[(13, 14)])),
            ("<script\nb", paragraphs(&[])),
            ("<scripts>\n\nb", paragraphs(&[(11, 12)])),
            ("a\n<x-y b='1' c=\"2\" d=e f/>\nc", paragraphs(&[(0, 28)])),
            (
                "<x-y b='1' c=\"2\" d=e f/>\nc\n\nd",
                paragraphs(&[(28, 29)]),
            ),
            ("</x >\n\nb", paragraphs(&[(7, 8)])),
            ("<x\n\nb", paragraphs(&[(0, 2), (4, 5)])),
            ("<x> y", paragraphs(&[(0, 5)])),
            ("<hr/>\na", paragraphs(&[])),
            ("<a b='c>\n\nd", paragraphs(&[(0, 8), (10, 11)])),
            ("<a b=c\u{1}d>\n\ne", paragraphs(&[(0, 9), (11, 12)])),
            ("a\n</div>\nb", paragraphs(&[(0, 1)])),
            ("a\n<hr/>\nb", paragraphs(&[(0, 1)])),
            ("a\n<div class=x>\nb", paragraphs(&[(0, 1)])),
            ("a\n<div\nb", paragraphs(&[(0, 1)])),
            ("<!a\nb", paragraphs(&[(0, 5)])),
            (
                "a|b\n-|-\n<x>",
                vec![(0, Kind::MarkdownTable, "a|b\n-|-\n<x>")],
            ),
            ("> a\n<x>", lazy.to_vec()),
            ("> <div>\na", quoted.to_vec()),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.iter().fold(
                HashMap::new(),
                |mut positions, &(position, kind, value)| {
                    positions
                        .entry(position)
                        .or_insert_with(Vec::new)
                        .push(Token::from((kind, value)));
                    positions
                },
            ));
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_inline_html_or_an_autolink_then_returns_no_code_span_inside_it() {
        let long_scheme = format!("x <{}:`>`", "a".repeat(33));
        let long_label = format!("x <a`b@{}>`", "c".repeat(64));
        let cases = [
            ("x <a b='`'>`", None),
            ("x <a b = \"`\" c d=e />`", None),
            ("x <a\nb='`'>`", None),
            ("x <a b=c\u{1}d e='`'>`", None),
            ("x <a b='`>`", Some((8, 10))),
            ("x <a b='c'd='`'>`", Some((13, 16))),
            ("x <a c='`' b=>`", Some((8, 14))),
            ("x <?`a`", Some((4, 6))),
            ("x <!--`a`", Some((6, 8))),
            ("x <![CDATA[`a`", Some((11, 13))),
            ("x <!X `a`", Some((6, 8))),
            ("x <![x`]]>`", Some((6, 10))),
            ("x <1a:b`>`", Some((7, 9))),
            (long_scheme.as_str(), Some((37, 39))),
            ("x <a_b:`>`", Some((7, 9))),
            ("x <a\"`b@c>`", Some((5, 10))),
            ("x <a`b@c_d>`", Some((4, 11))),
            ("x <a b1='`'>`", None),
            ("x <a _b='`'>`", None),
            ("x <a b=c\td='`'>`", None),
            ("x <a`b@c->`", Some((4, 10))),
            (long_label.as_str(), Some((4, 72))),
            ("x <a`b>`", Some((4, 7))),
            ("x <!-- ` -->`", None),
            ("x <!--->`a`-->", Some((8, 10))),
            ("x <!-->`a`-->", Some((7, 9))),
            ("x <!->`a`", Some((6, 8))),
            ("x <?`?>`", None),
            ("x <??`?>`", Some((5, 8))),
            ("x <?a??`?>`", Some((7, 10))),
            ("x <![CDATA[`]]>`", None),
            ("x <!X `>`", None),
            ("x <!X`>`", Some((5, 7))),
            ("x <!1>`a`", Some((6, 8))),
            ("x <!", None),
            ("x <", None),
            ("x <http://a`b>`", None),
            ("x <a`b@c.d>`", None),
            ("x <a`b@-c>`", Some((4, 10))),
            ("x <a:b`>`", Some((6, 8))),
            ("x <ab: `>`", Some((7, 9))),
            ("x <>`a`", Some((4, 6))),
            ("`<a b='`'>`", Some((0, 7))),
            ("[a](<b `c>)`", None),
            ("[a](<b\n`c>)`", Some((7, 11))),
        ];
        for (text, span) in cases {
            let positions = parse(text);

            let tick = |kind: Kind| Token::from((kind, "`"));
            let edges = [
                (0, Token::from((Kind::ParagraphOpen, ""))),
                (text.len(), Token::from((Kind::ParagraphClose, ""))),
            ];
            let spans = span.into_iter().flat_map(|(start, end)| {
                [
                    (start, tick(Kind::MarkdownCodeSpanStart)),
                    (end, tick(Kind::MarkdownCodeSpanEnd)),
                ]
            });
            let expected = Positions(edges.into_iter().chain(spans).fold(
                HashMap::new(),
                |mut positions, (position, token)| {
                    positions
                        .entry(position)
                        .or_insert_with(Vec::new)
                        .push(token);
                    positions
                },
            ));
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_null_character_then_returns_positions_in_the_original_text() {
        let token = |kind: Kind, value: &str| Token::from((kind, value));
        let cases = [
            (
                "a\0`b`",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (2, vec![token(Kind::MarkdownCodeSpanStart, "`")]),
                    (4, vec![token(Kind::MarkdownCodeSpanEnd, "`")]),
                    (5, vec![token(Kind::ParagraphClose, "")]),
                ],
            ),
            (
                "\0\n```\0\n\0\n```",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (1, vec![token(Kind::ParagraphClose, "")]),
                    (
                        2,
                        vec![
                            token(Kind::MarkdownFencedCodeBlock, "```\0\n\0\n```"),
                            token(Kind::FenceStart, "```\u{FFFD}"),
                            token(Kind::FenceLiteral, "\u{FFFD}\n"),
                            token(Kind::FenceEnd, "```"),
                        ],
                    ),
                ],
            ),
            (
                "\0\n\n\0",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (1, vec![token(Kind::ParagraphClose, "")]),
                    (3, vec![token(Kind::ParagraphOpen, "")]),
                    (4, vec![token(Kind::ParagraphClose, "")]),
                ],
            ),
            (
                "\0\0\0\0`a`",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (4, vec![token(Kind::MarkdownCodeSpanStart, "`")]),
                    (6, vec![token(Kind::MarkdownCodeSpanEnd, "`")]),
                    (7, vec![token(Kind::ParagraphClose, "")]),
                ],
            ),
            (
                "| \0 |\n|---|\n\na",
                vec![
                    (0, vec![token(Kind::MarkdownTable, "| \0 |\n|---|")]),
                    (13, vec![token(Kind::ParagraphOpen, "")]),
                    (14, vec![token(Kind::ParagraphClose, "")]),
                ],
            ),
            (
                "\0\n\n| a |\n|---|",
                vec![
                    (0, vec![token(Kind::ParagraphOpen, "")]),
                    (1, vec![token(Kind::ParagraphClose, "")]),
                    (3, vec![token(Kind::MarkdownTable, "| a |\n|---|")]),
                ],
            ),
            (
                "```\0\n```\na",
                vec![
                    (
                        0,
                        vec![
                            token(Kind::MarkdownFencedCodeBlock, "```\0\n```"),
                            token(Kind::FenceStart, "```\u{FFFD}"),
                            token(Kind::FenceLiteral, ""),
                            token(Kind::FenceEnd, "```"),
                        ],
                    ),
                    (9, vec![token(Kind::ParagraphOpen, "")]),
                    (10, vec![token(Kind::ParagraphClose, "")]),
                ],
            ),
            (
                "| \0 |\n|---|",
                vec![(0, vec![token(Kind::MarkdownTable, "| \0 |\n|---|")])],
            ),
        ];
        for (text, expected) in cases {
            let positions = parse(text);

            let expected = Positions(expected.into_iter().collect());
            assert_eq!(positions, Some(Ok(expected)), "{text:?}");
        }
    }

    #[test]
    fn when_parse_with_a_null_character_in_a_link_label_then_returns_unsupported() {
        let text = "[\0]: b";

        let positions = parse(text);

        assert_eq!(positions, Some(Err(Unsupported)));
    }

    #[test]
    fn when_parse_with_a_fence_info_holding_escapes_or_entities_then_returns_it_decoded() {
        let cases = [
            ("a&amp;b", "a&b"),
            ("a&AMP;b", "a&b"),
            ("a&Amp;b", "a&Amp;b"),
            ("a&amp b", "a&amp b"),
            ("a&ngE;b", "a\u{2267}\u{338}b"),
            ("a&NewLine;b", "a\nb"),
            ("a&#65;b", "aAb"),
            ("a&#x41;b", "aAb"),
            ("a&#X4a;b", "aJb"),
            ("a&#0;b", "a\u{FFFD}b"),
            ("a&#x110000;b", "a\u{FFFD}b"),
            ("a&#9999999;b", "a\u{FFFD}b"),
            ("a&#12345678;b", "a&#12345678;b"),
            ("a&#x1234567;b", "a&#x1234567;b"),
            ("a&#;b", "a&#;b"),
            ("a&#x;b", "a&#x;b"),
            ("a&#65b", "a&#65b"),
            ("a&#xD83D;&#xDE00;b", "a\u{1F600}b"),
            ("a&frac12;b", "a\u{BD}b"),
            ("a\\&amp;b", "a&amp;b"),
            ("a\\\\&amp;b", "a\\&b"),
            ("a\\qb", "a\\qb"),
            ("a\\", "a\\"),
        ];
        for (info, decoded) in cases {
            let text = format!("```{info}");

            let positions = parse(&text);

            let token = |kind: Kind, value: &str| Token::from((kind, value));
            let expected = Positions(
                [(
                    0,
                    vec![
                        token(Kind::MarkdownFencedCodeBlock, &text),
                        token(Kind::FenceStart, &format!("```{decoded}")),
                        token(Kind::FenceLiteral, ""),
                        token(Kind::FenceEnd, "```"),
                    ],
                )]
                .into_iter()
                .collect(),
            );
            assert_eq!(positions, Some(Ok(expected)), "{info:?}");
        }
    }
}
