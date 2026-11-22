use std::collections::HashMap;

use super::doc::{Break, BreakMode, BreakTag, Doc, Group, HeaderMark, Indent};
use super::input::{CommentKind, Input, Tok, TokKind, Token};
use super::output::BlankLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reality {
    Real,
    Imaginary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op<'s> {
    Open(Indent),
    Close,
    Break {
        mode: BreakMode,
        flat: &'s str,
        plus_indent: Indent,
        tag: Option<BreakTag>,
    },
    Space,
    Token {
        text: &'s str,
        comments_indent: Indent,
        trailing: Option<Indent>,
    },
    Fence,
    BlankLine(BlankLine),
    Sync(usize),
    Header(HeaderMark),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unformatted {
    Deferred,
    Rejected {
        offset: usize,
        message: String,
    },
    RejectedAt {
        line: usize,
        column: usize,
        message: String,
    },
}

#[derive(Debug, Clone)]
enum Event<'a> {
    Open(Indent),
    Close,
    Break(Break),
    Token {
        token: &'a Token,
        reality: Reality,
        comments_indent: Indent,
        trailing: Option<Indent>,
    },
    Tok(&'a Tok),
    Space,
    Fence,
    Header(HeaderMark),
}

enum Item {
    Doc(Doc),
    Level(usize),
}

struct Level {
    indent: Indent,
    items: Vec<Item>,
}

pub struct Builder<'a> {
    pub(super) input: &'a Input,
    events: Vec<Event<'a>>,
    pub(super) next: usize,
    tags: usize,
    blank_lines: HashMap<usize, BlankLine>,
    input_position: usize,
}

impl<'a> From<&'a Input> for Builder<'a> {
    fn from(input: &'a Input) -> Self {
        Builder {
            input,
            events: Vec::new(),
            next: 0,
            tags: 0,
            blank_lines: HashMap::new(),
            input_position: 0,
        }
    }
}

impl<'a> Builder<'a> {
    pub fn tag(&mut self) -> BreakTag {
        let tag = BreakTag(self.tags);
        self.tags += 1;
        tag
    }

    pub fn add(&mut self, op: Op) -> Result<(), Unformatted> {
        match op {
            Op::Open(indent) => self.events.push(Event::Open(indent)),
            Op::Close => self.events.push(Event::Close),
            Op::Break {
                mode,
                flat,
                plus_indent,
                tag,
            } => self.events.push(Event::Break(Break {
                mode,
                flat: flat.to_owned(),
                plus_indent,
                tag,
            })),
            Op::Space => self.events.push(Event::Space),
            Op::Sync(position) => self.input_position = self.input_position.max(position),
            Op::Fence => self.events.push(Event::Fence),
            Op::Header(mark) => self.events.push(Event::Header(mark)),
            Op::Token {
                text,
                comments_indent,
                trailing,
            } => match self.input.tokens.get(self.next) {
                Some(token) if token.tok.text == text => {
                    self.next += 1;
                    self.events.push(Event::Token {
                        token,
                        reality: Reality::Real,
                        comments_indent,
                        trailing,
                    });
                }
                Some(token) => {
                    return Err(Unformatted::Rejected {
                        offset: self.input_position,
                        message: format!(
                            "expected token: '{}'; generated {text} instead",
                            token.tok.text
                        ),
                    });
                }
                None => {
                    return Err(Unformatted::Rejected {
                        offset: self.input_position,
                        message: format!("expected token: 'null'; generated {text} instead"),
                    });
                }
            },
            Op::BlankLine(wanted) => {
                let token = self
                    .input
                    .tokens
                    .get(self.next)
                    .ok_or(Unformatted::Rejected {
                        offset: self.input_position,
                        message: "blank line requested after the end of the input".to_owned(),
                    })?;
                let k = token
                    .before
                    .iter()
                    .find_map(Tok::index)
                    .unwrap_or(token.index);
                let merged = match self.blank_lines.remove(&k) {
                    Some(existing) => existing.merge(wanted),
                    None => wanted,
                };
                self.blank_lines.insert(k, merged);
            }
        }
        Ok(())
    }

    pub fn build(mut self) -> Result<(Doc, HashMap<usize, BlankLine>), Unformatted> {
        for token in &self.input.tokens[self.next..] {
            self.events.push(Event::Token {
                token,
                reality: Reality::Imaginary,
                comments_indent: Indent::Const(0),
                trailing: None,
            });
        }
        let events = self.events;
        let forced = |plus_indent: Indent| Break {
            mode: BreakMode::Forced,
            flat: String::new(),
            plus_indent,
            tag: None,
        };
        let mut blank_lines = self.blank_lines;
        let n = events.len();
        let mut around: Vec<Vec<Event>> = vec![Vec::new(); n + 1];
        for (i, event) in events.iter().enumerate() {
            let Event::Token {
                token,
                reality,
                comments_indent,
                trailing,
            } = event
            else {
                continue;
            };
            let mut first = i;
            while first > 0 && matches!(events[first - 1], Event::Open(_)) {
                first -= 1;
            }
            let mut last = i;
            while last + 1 < n && matches!(events[last + 1], Event::Close) {
                last += 1;
            }
            let mut before = Vec::new();
            let mut after = Vec::new();
            match reality {
                Reality::Real => {
                    let mut newlines = 0;
                    let mut space = false;
                    let mut last_was_comment = false;
                    for tok in &token.before {
                        let comment = match tok.kind {
                            TokKind::Newline => {
                                newlines += 1;
                                continue;
                            }
                            TokKind::Token(_) | TokKind::Space => continue,
                            TokKind::Comment(kind, _) => kind,
                        };
                        let slash_slash = match comment {
                            CommentKind::Line | CommentKind::MarkdownDoc => true,
                            CommentKind::Block | CommentKind::Doc => false,
                        };
                        let mode = match slash_slash {
                            true => BreakMode::Forced,
                            false => BreakMode::Unified,
                        };
                        before.push(Event::Break(Break {
                            mode,
                            flat: String::new(),
                            plus_indent: comments_indent.clone(),
                            tag: None,
                        }));
                        before.push(Event::Tok(tok));
                        if slash_slash {
                            before.push(Event::Break(forced(Indent::Const(0))));
                        }
                        space = !slash_slash;
                        newlines = 0;
                        last_was_comment = true;
                    }
                    if last_was_comment && newlines > 1 {
                        let index = token.index;
                        let merged = match blank_lines.remove(&index) {
                            Some(existing) => existing.merge(BlankLine::Yes),
                            None => BlankLine::Yes,
                        };
                        blank_lines.insert(index, merged);
                    }
                    match (last_was_comment && newlines > 0, space) {
                        (true, _) => before.push(Event::Break(forced(Indent::Const(0)))),
                        (false, true) => before.push(Event::Space),
                        (false, false) => {}
                    }
                    for tok in &token.after {
                        let TokKind::Comment(comment, _) = tok.kind else {
                            continue;
                        };
                        let slash_slash = match comment {
                            CommentKind::Line | CommentKind::MarkdownDoc => true,
                            CommentKind::Block | CommentKind::Doc => false,
                        };
                        let break_after = !slash_slash && trailing.is_some();
                        match break_after {
                            true => after.push(Event::Break(forced(
                                trailing.clone().unwrap_or(Indent::Const(0)),
                            ))),
                            false => after.push(Event::Space),
                        }
                        after.push(Event::Tok(tok));
                        if break_after || slash_slash {
                            after.push(Event::Break(forced(Indent::Const(0))));
                        }
                    }
                }
                Reality::Imaginary => {
                    let mut newlines = 0;
                    let mut last_was_comment = false;
                    for tok in &token.before {
                        match tok.kind {
                            TokKind::Newline => newlines += 1,
                            TokKind::Comment(..) => {
                                newlines = 0;
                                last_was_comment = true;
                            }
                            TokKind::Token(_) | TokKind::Space => {}
                        }
                        if last_was_comment && newlines > 0 {
                            before.push(Event::Break(forced(Indent::Const(0))));
                        }
                        before.push(Event::Tok(tok));
                    }
                    after.extend(token.after.iter().map(Event::Tok));
                }
            }
            around[first].extend(before);
            around[last + 1].extend(after);
        }

        let is_forced = |event: &Event| {
            matches!(
                event,
                Event::Break(Break {
                    mode: BreakMode::Forced,
                    ..
                })
            )
        };
        let mut spliced = Vec::new();
        let mut after_forced = false;
        for (i, extra) in around.into_iter().enumerate() {
            for event in extra {
                if !(after_forced && matches!(event, Event::Space)) {
                    after_forced = is_forced(&event);
                    spliced.push(event);
                }
            }
            let Some(event) = events.get(i) else {
                continue;
            };
            let space_like = match event {
                Event::Space => true,
                Event::Break(separator) => {
                    separator.plus_indent.eval(&[]) == 0 && separator.flat == " "
                }
                Event::Open(_)
                | Event::Close
                | Event::Token { .. }
                | Event::Tok(_)
                | Event::Fence
                | Event::Header(_) => false,
            };
            if after_forced && space_like {
                continue;
            }
            spliced.push(event.clone());
            match event {
                Event::Open(_) | Event::Fence | Event::Header(_) => {}
                Event::Close
                | Event::Break(_)
                | Event::Token { .. }
                | Event::Tok(_)
                | Event::Space => after_forced = is_forced(event),
            }
        }

        let unbalanced = Unformatted::Rejected {
            offset: self.input_position,
            message: "unbalanced close".to_owned(),
        };
        let mut levels = vec![Level {
            indent: Indent::Const(0),
            items: Vec::new(),
        }];
        let mut stack = vec![0];
        let mut append = 0;
        for event in spliced {
            match event {
                Event::Open(indent) => {
                    levels.push(Level {
                        indent,
                        items: Vec::new(),
                    });
                    stack.push(levels.len() - 1);
                }
                Event::Close => {
                    let top = stack.pop().ok_or(unbalanced.clone())?;
                    let parent = *stack.last().ok_or(unbalanced.clone())?;
                    levels[parent].items.push(Item::Level(top));
                }
                Event::Break(separator) => {
                    append = *stack.last().ok_or(unbalanced.clone())?;
                    levels[append].items.push(Item::Doc(Doc::Break(separator)));
                }
                Event::Token { token, .. } => {
                    let range = Some((token.index, token.index + 1));
                    levels[append]
                        .items
                        .push(Item::Doc(Doc::Text(token.tok.text.clone(), range)));
                }
                Event::Tok(tok) => {
                    let range = tok.index().map(|index| (index, index + 1));
                    levels[append]
                        .items
                        .push(Item::Doc(Doc::comment(&tok.text, range)));
                }
                Event::Space => levels[append].items.push(Item::Doc(Doc::Space)),
                Event::Fence => {}
                Event::Header(mark) => levels[append].items.push(Item::Doc(Doc::Header(mark))),
            }
        }
        if stack.len() != 1 {
            return Err(Unformatted::Rejected {
                offset: self.input_position,
                message: format!("saw {} unclosed ops", stack.len() - 1),
            });
        }
        let mut docs: Vec<Option<Doc>> = vec![None; levels.len()];
        for (index, level) in levels.into_iter().enumerate().rev() {
            let mut children = Vec::new();
            for item in level.items {
                match item {
                    Item::Doc(doc) => children.push(doc),
                    Item::Level(child) => {
                        children.push(docs[child].take().ok_or(unbalanced.clone())?)
                    }
                }
            }
            docs[index] = Some(Doc::Group(Group::new(level.indent, children)));
        }
        let root = docs[0].take().ok_or(unbalanced)?;
        Ok((root, blank_lines))
    }
}

#[cfg(test)]
mod tests {
    use super::super::comments;
    use super::super::doc;
    use super::super::input;
    use super::super::output::{JavaOutput, Pending};
    use super::*;
    use crate::lexer;

    #[test]
    fn when_build_with_ops_then_returns_the_layout_gjf_builds_for_them() {
        let token = |text: &'static str| Op::Token {
            text,
            comments_indent: Indent::Const(0),
            trailing: None,
        };
        let forced = Op::Break {
            mode: BreakMode::Forced,
            flat: "",
            plus_indent: Indent::Const(0),
            tag: None,
        };
        let cases: [(&str, Vec<Op>, &str); 8] = [
            (
                "a b",
                vec![token("a"), forced.clone(), Op::Space, token("b")],
                "a\nb\n",
            ),
            (
                "/* c */ a",
                vec![Op::Open(Indent::Const(0)), token("a"), Op::Close],
                "/* c */ a\n",
            ),
            ("// c\na", vec![token("a")], "// c\na\n"),
            (
                "a // c\nb",
                vec![token("a"), forced.clone(), token("b")],
                "a // c\nb\n",
            ),
            ("// c\n\na", vec![token("a")], "// c\n\na\n"),
            (
                "a\n// end\n",
                vec![token("a"), forced.clone()],
                "a\n// end\n",
            ),
            (
                "a\n\nb",
                vec![
                    token("a"),
                    forced.clone(),
                    Op::BlankLine(BlankLine::Preserve),
                    Op::BlankLine(BlankLine::Yes),
                    token("b"),
                ],
                "a\n\nb\n",
            ),
            (
                "a /* c */\nb",
                vec![
                    Op::Token {
                        text: "a",
                        comments_indent: Indent::Const(0),
                        trailing: Some(Indent::Const(2)),
                    },
                    forced.clone(),
                    token("b"),
                    Op::Fence,
                ],
                "a\n  /* c */\nb\n",
            ),
        ];
        for (source, ops, expected) in cases {
            let tokens = lexer::scan(source)
                .map(|lexed| lexed.tokens)
                .unwrap_or_default();
            let input = input::build(source, &tokens);
            let mut builder = Builder::from(&input);

            let built = ops
                .into_iter()
                .try_for_each(|op| builder.add(op))
                .and_then(|()| builder.build());

            let rendered = built.map(|(doc, blank_lines)| {
                let layout = doc::solve(&doc, 100, &comments::rewrite);
                let mut output = JavaOutput {
                    input_lines: input.line_ranges.clone(),
                    blank_lines,
                    taken: layout.taken.clone(),
                    pending: Pending::default(),
                };
                layout.replay(&mut output);
                String::from(output)
            });
            assert_eq!(rendered, Ok(expected.to_owned()), "{source:?}");
        }
    }

    #[test]
    fn when_build_with_a_token_the_input_does_not_have_then_returns_gjf_rejection() {
        let cases = [
            (
                "a",
                "b",
                Unformatted::Rejected {
                    offset: 0,
                    message: "expected token: 'a'; generated b instead".to_owned(),
                },
            ),
            (
                "",
                "b",
                Unformatted::Rejected {
                    offset: 0,
                    message: "expected token: ''; generated b instead".to_owned(),
                },
            ),
        ];
        for (source, text, expected) in cases {
            let tokens = lexer::scan(source)
                .map(|lexed| lexed.tokens)
                .unwrap_or_default();
            let input = input::build(source, &tokens);
            let mut builder = Builder::from(&input);

            let added = builder.add(Op::Token {
                text,
                comments_indent: Indent::Const(0),
                trailing: None,
            });

            assert_eq!(added, Err(expected), "{source:?}");
        }
    }
}
