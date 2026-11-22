use crate::lexer::token::{Token as LexToken, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tok {
    pub text: String,
    pub position: usize,
    pub column: usize,
    pub kind: TokKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokKind {
    Token(usize),
    Space,
    Newline,
    Comment(CommentKind, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub before: Vec<Tok>,
    pub tok: Tok,
    pub index: usize,
    pub after: Vec<Tok>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub tokens: Vec<Token>,
    pub line_ranges: Vec<Option<(usize, usize)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    MarkdownDoc,
    Block,
    Doc,
}

impl Tok {
    pub fn index(&self) -> Option<usize> {
        match self.kind {
            TokKind::Token(index) | TokKind::Comment(_, index) => Some(index),
            TokKind::Space | TokKind::Newline => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Piece {
    Token,
    Space,
    Newline,
    Comment(CommentKind),
}

pub fn build(text: &str, tokens: &[LexToken]) -> Input {
    let param_comment = |text: &str| {
        text.strip_prefix("/*")
            .and_then(|t| t.strip_suffix("*/"))
            .and_then(|inner| {
                inner
                    .trim_end_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r'])
                    .strip_suffix('=')
            })
            .is_some_and(|before| {
                !before.is_empty()
                    && before.chars().all(|c| {
                        c.is_ascii_alphanumeric()
                            || matches!(c, '_' | '-' | ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
                    })
            })
    };
    let utf16 = |s: &str| s.encode_utf16().count();
    let characters = |original: &str, start: usize| -> Vec<(String, usize, Piece)> {
        let mut pieces = Vec::new();
        for (offset, c) in original.char_indices() {
            for unit in c.encode_utf16(&mut [0; 2]).iter() {
                let piece =
                    char::from_u32(u32::from(*unit)).map_or("?".to_owned(), |c| c.to_string());
                pieces.push((piece, start + offset, Piece::Token));
            }
        }
        pieces
    };
    let mut toks: Vec<Tok> = Vec::new();
    let mut numbered = 0;
    let mut column = 0;
    for token in tokens {
        let original = &text[token.start..token.end];
        let whole = |piece: Piece| vec![(original.to_owned(), token.start, piece)];
        let escaped = original.starts_with('\\');
        let pieces = match (escaped, token.kind) {
            (_, TokenKind::StringLiteral | TokenKind::TextBlock) => whole(Piece::Token),
            (true, _) | (false, TokenKind::Punct(_)) => characters(original, token.start),
            (false, TokenKind::Whitespace) => whole(Piece::Space),
            (false, TokenKind::Newline) => whole(Piece::Newline),
            (
                false,
                TokenKind::LineComment | TokenKind::BlockComment | TokenKind::JavadocComment,
            ) => {
                let doc = original.len() > 4;
                let kind = match (
                    &original[..2],
                    doc && original.starts_with("///") && !original.starts_with("////"),
                    doc && original.starts_with("/**") && !original.starts_with("/***"),
                ) {
                    ("//", true, _) => CommentKind::MarkdownDoc,
                    ("//", false, _) => CommentKind::Line,
                    (_, _, true) => CommentKind::Doc,
                    (_, _, false) => CommentKind::Block,
                };
                whole(Piece::Comment(kind))
            }
            (false, TokenKind::CharLiteral) => whole(Piece::Token),
            (
                false,
                TokenKind::Identifier
                | TokenKind::Keyword(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::IntegerLiteral
                | TokenKind::FloatingLiteral
                | TokenKind::Eof,
            ) => match original.chars().next() {
                Some(first) if u32::from(first) <= 0xFFFF => whole(Piece::Token),
                Some(_) | None => characters(original, token.start),
            },
        };
        for (piece_text, position, piece) in pieces {
            let kind = match piece {
                Piece::Token => TokKind::Token(numbered),
                Piece::Comment(comment) => TokKind::Comment(comment, numbered),
                Piece::Space => TokKind::Space,
                Piece::Newline => TokKind::Newline,
            };
            if let Piece::Token | Piece::Comment(_) = piece {
                numbered += 1;
            }
            toks.push(Tok {
                text: piece_text,
                position,
                column,
                kind,
            });
            column = match original.rfind(['\n', '\r']) {
                Some(last) => utf16(&original[last + 1..]),
                None => column + utf16(original),
            };
        }
    }
    toks.push(Tok {
        text: String::new(),
        position: text.len(),
        column,
        kind: TokKind::Token(numbered),
    });

    let line_count = match (text.is_empty(), text.ends_with('\n')) {
        (true, _) => 0,
        (false, true) => text.matches('\n').count(),
        (false, false) => text.matches('\n').count() + 1,
    };
    let mut line_ranges: Vec<Option<(usize, usize)>> = vec![None; line_count];
    let mut line = 0;
    for tok in &toks {
        let first = line;
        line += tok.text.matches('\n').count();
        if let Some(index) = tok.index() {
            if line_ranges.len() <= line {
                line_ranges.resize(line + 1, None);
            }
            for slot in line_ranges.iter_mut().take(line + 1).skip(first) {
                *slot = Some(match *slot {
                    Some((low, high)) => (low.min(index), high.max(index + 1)),
                    None => (index, index + 1),
                });
            }
        }
    }

    let numbered_tok = |tok: &Tok| match tok.kind {
        TokKind::Token(index) => Some(index),
        TokKind::Space | TokKind::Newline | TokKind::Comment(..) => None,
    };
    let mut built = Vec::new();
    let mut before: Vec<Tok> = Vec::new();
    let mut k = 0;
    'outermost: while k < toks.len() {
        while let Some(tok) = toks.get(k).filter(|tok| numbered_tok(tok).is_none()) {
            let param = param_comment(&tok.text);
            before.push(tok.clone());
            k += 1;
            if param {
                while toks.get(k).is_some_and(|tok| tok.kind == TokKind::Newline) {
                    k += 1;
                }
            }
        }
        let Some((tok, index)) = toks
            .get(k)
            .and_then(|tok| numbered_tok(tok).map(|index| (tok.clone(), index)))
        else {
            break;
        };
        k += 1;
        let mut after = Vec::new();
        while let Some(next) = toks.get(k).filter(|tok| numbered_tok(tok).is_none()) {
            let stops = matches!(
                (next.kind, tok.text.as_str()),
                (
                    TokKind::Comment(CommentKind::Block | CommentKind::Doc, _),
                    "(" | "<" | "."
                ) | (
                    TokKind::Comment(CommentKind::Doc | CommentKind::MarkdownDoc, _),
                    ";"
                )
            );
            if stops {
                break;
            }
            if param_comment(&next.text) {
                built.push(Token {
                    before: std::mem::take(&mut before),
                    tok: tok.clone(),
                    index,
                    after: std::mem::take(&mut after),
                });
                before.push(next.clone());
                k += 1;
                while toks.get(k).is_some_and(|tok| tok.kind == TokKind::Newline) {
                    k += 1;
                }
                continue 'outermost;
            }
            after.push(next.clone());
            k += 1;
            if next.text.contains(['\n', '\r']) {
                break;
            }
        }
        built.push(Token {
            before: std::mem::take(&mut before),
            tok,
            index,
            after,
        });
    }
    Input {
        tokens: built,
        line_ranges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    type Expected<'s> = (&'s [&'s str], &'s str, &'s [&'s str]);

    #[test]
    fn when_build_with_comments_and_split_tokens_then_returns_tokens_as_java_input_does() {
        let cases: [(&str, &[Expected]); 9] = [
            (
                "a /* c */ b",
                &[
                    (&[], "a", &[" ", "/* c */", " "]),
                    (&[], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "f(/* c */x)",
                &[
                    (&[], "f", &[]),
                    (&[], "(", &[]),
                    (&["/* c */"], "x", &[]),
                    (&[], ")", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "a;/** d */\nb",
                &[
                    (&[], "a", &[]),
                    (&[], ";", &[]),
                    (&["/** d */", "\n"], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "f(/* x= */\n1)",
                &[
                    (&[], "f", &[]),
                    (&[], "(", &[]),
                    (&["/* x= */"], "1", &[]),
                    (&[], ")", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "a /* y= */\nb",
                &[
                    (&[], "a", &[" "]),
                    (&["/* y= */"], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "a // c\nb",
                &[
                    (&[], "a", &[" ", "// c", "\n"]),
                    (&[], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "\\u0061b",
                &[
                    (&[], "\\", &[]),
                    (&[], "u", &[]),
                    (&[], "0", &[]),
                    (&[], "0", &[]),
                    (&[], "6", &[]),
                    (&[], "1", &[]),
                    (&[], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "a>>=b",
                &[
                    (&[], "a", &[]),
                    (&[], ">", &[]),
                    (&[], ">", &[]),
                    (&[], "=", &[]),
                    (&[], "b", &[]),
                    (&[], "", &[]),
                ],
            ),
            (
                "\u{1d465}",
                &[(&[], "?", &[]), (&[], "?", &[]), (&[], "", &[])],
            ),
        ];
        for (source, expected) in cases {
            let tokens = lexer::scan(source)
                .map(|lexed| lexed.tokens)
                .unwrap_or_default();

            let input = build(source, &tokens);

            let texts =
                |toks: &[Tok]| -> Vec<String> { toks.iter().map(|tok| tok.text.clone()).collect() };
            let actual: Vec<(Vec<String>, String, Vec<String>)> = input
                .tokens
                .iter()
                .map(|token| {
                    (
                        texts(&token.before),
                        token.tok.text.clone(),
                        texts(&token.after),
                    )
                })
                .collect();
            let expected: Vec<(Vec<String>, String, Vec<String>)> = expected
                .iter()
                .map(|(before, tok, after)| {
                    (
                        before.iter().map(|s| (*s).to_owned()).collect(),
                        (*tok).to_owned(),
                        after.iter().map(|s| (*s).to_owned()).collect(),
                    )
                })
                .collect();
            assert_eq!(actual, expected, "{source:?}");
        }
    }

    #[test]
    fn when_build_with_lines_then_returns_the_token_range_of_each_line() {
        let cases = [
            (
                "a\n\nb\n",
                vec![Some((0, 1)), None, Some((1, 2)), Some((2, 3))],
            ),
            ("/* a\nb */ c", vec![Some((0, 1)), Some((0, 3))]),
            ("", vec![Some((0, 1))]),
        ];
        for (source, expected) in cases {
            let tokens = lexer::scan(source)
                .map(|lexed| lexed.tokens)
                .unwrap_or_default();

            let input = build(source, &tokens);

            assert_eq!(input.line_ranges, expected, "{source:?}");
        }
    }
}
