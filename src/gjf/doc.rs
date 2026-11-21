use crate::lexer;
use crate::lexer::token::{Token, TokenKind};

pub const MAX_LINE_WIDTH: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    Unified,
    Independent,
    Forced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakTag(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indent {
    Const(i32),
    If(BreakTag, Box<Indent>, Box<Indent>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Break {
    id: usize,
    fill: FillMode,
    flat: String,
    plus_indent: Indent,
    tag: Option<BreakTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    Level {
        id: usize,
        plus_indent: Indent,
        docs: Vec<Doc>,
        width: usize,
        flat: String,
    },
    Token(String),
    Space,
    Break(Break),
    Comment {
        id: usize,
        text: String,
        flat: String,
        width: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub last_indent: i32,
    pub indent: i32,
    pub column: usize,
    pub must_break: bool,
}

pub trait Comments {
    fn rewrite(&self, text: &str, max_width: usize, column: usize) -> String;
}

impl<F: Fn(&str, usize, usize) -> String> Comments for F {
    fn rewrite(&self, text: &str, max_width: usize, column: usize) -> String {
        self(text, max_width, column)
    }
}

pub trait Sink {
    fn append(&mut self, text: &str);
    fn indent(&mut self, indent: usize);
}

#[derive(Debug)]
pub struct Layout {
    one_line: Vec<bool>,
    broken: Vec<Option<usize>>,
    tags: Vec<Option<bool>>,
    comments: Vec<String>,
}

#[derive(Default)]
pub struct Builder {
    next_level: usize,
    next_break: usize,
    next_comment: usize,
    next_tag: usize,
}

pub fn parameter_comment(text: &str) -> Option<String> {
    let java_space = |c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r');
    let inner = text.strip_prefix("/*")?.strip_suffix("*/")?;
    let name = inner
        .trim_start_matches(java_space)
        .strip_suffix(|c| c == '=')
        .map(|n| n.trim_end_matches(java_space));
    let name = name.or_else(|| {
        let before = inner.trim_end_matches(java_space).strip_suffix('=')?;
        Some(before.trim_matches(java_space))
    })?;
    let identifier = name.strip_suffix("...").unwrap_or(name);
    let tokens = lexer::scan(identifier).ok()?;
    match (tokens.as_slice(), identifier.contains('\\')) {
        (
            [
                Token {
                    kind:
                        TokenKind::Identifier
                        | TokenKind::Keyword(_)
                        | TokenKind::True
                        | TokenKind::False
                        | TokenKind::Null,
                    ..
                },
            ],
            false,
        ) => Some(format!("/* {name}= */")),
        _ => None,
    }
}

impl Break {
    fn width(&self) -> usize {
        match self.fill {
            FillMode::Forced => MAX_LINE_WIDTH,
            FillMode::Unified | FillMode::Independent => self.flat.encode_utf16().count(),
        }
    }
}

impl Doc {
    pub fn width(&self) -> usize {
        match self {
            Doc::Level { width, .. } | Doc::Comment { width, .. } => *width,
            Doc::Token(text) => match text.contains(['\n', '\r']) {
                true => MAX_LINE_WIDTH,
                false => text.encode_utf16().count(),
            },
            Doc::Space => 1,
            Doc::Break(separator) => separator.width(),
        }
    }

    pub fn flat(&self) -> &str {
        match self {
            Doc::Level { flat, .. } | Doc::Token(flat) | Doc::Comment { flat, .. } => flat,
            Doc::Break(Break { flat, .. }) => flat,
            Doc::Space => " ",
        }
    }
}

impl Builder {
    pub fn tag(&mut self) -> BreakTag {
        self.next_tag += 1;
        BreakTag(self.next_tag - 1)
    }

    pub fn level(&mut self, plus_indent: Indent, docs: Vec<Doc>) -> Doc {
        let mut width = 0;
        for doc in &docs {
            width = (width + doc.width()).min(MAX_LINE_WIDTH);
        }
        let flat: String = docs.iter().map(Doc::flat).collect();
        self.next_level += 1;
        Doc::Level {
            id: self.next_level - 1,
            plus_indent,
            docs,
            width,
            flat,
        }
    }

    pub fn break_doc(
        &mut self,
        fill: FillMode,
        flat: &str,
        plus_indent: Indent,
        tag: Option<BreakTag>,
    ) -> Doc {
        self.next_break += 1;
        Doc::Break(Break {
            id: self.next_break - 1,
            fill,
            flat: flat.to_owned(),
            plus_indent,
            tag,
        })
    }

    pub fn comment(&mut self, text: &str) -> Doc {
        let parameter = parameter_comment(text);
        let unspaced = text.starts_with("//") && !text.starts_with("// ");
        let first_break = text.find(['\n', '\r']);
        let width = match (first_break, unspaced, &parameter) {
            (Some(index), _, _) if index > 0 => text[..index].encode_utf16().count(),
            (_, true, _) => text.encode_utf16().count() + 1,
            (_, false, Some(reformatted)) => reformatted.encode_utf16().count(),
            (_, false, None) => text.encode_utf16().count(),
        };
        let flat = match (unspaced, parameter) {
            (true, _) => format!("// {}", &text[2..]),
            (false, Some(reformatted)) => reformatted,
            (false, None) => text.to_owned(),
        };
        self.next_comment += 1;
        Doc::Comment {
            id: self.next_comment - 1,
            text: text.to_owned(),
            flat,
            width,
        }
    }
}

impl From<&Builder> for Layout {
    fn from(builder: &Builder) -> Self {
        Layout {
            one_line: vec![false; builder.next_level],
            broken: vec![None; builder.next_break],
            tags: vec![None; builder.next_tag],
            comments: vec![String::new(); builder.next_comment],
        }
    }
}

impl Indent {
    pub fn eval(&self, layout: &Layout) -> i32 {
        match self {
            Indent::Const(n) => *n,
            Indent::If(tag, then, otherwise) => match layout.tags.get(tag.0).copied().flatten() {
                Some(true) => then.eval(layout),
                Some(false) | None => otherwise.eval(layout),
            },
        }
    }
}

impl Layout {
    pub fn compute(
        &mut self,
        doc: &Doc,
        comments: &dyn Comments,
        max_width: usize,
        state: State,
    ) -> State {
        match doc {
            Doc::Level {
                id,
                plus_indent,
                docs,
                width,
                ..
            } => {
                if state.column + width <= max_width {
                    self.one_line[*id] = true;
                    return State {
                        column: state.column + width,
                        ..state
                    };
                }
                let indent = state.indent + plus_indent.eval(self);
                let mut inner = State {
                    last_indent: indent,
                    indent,
                    column: state.column,
                    must_break: false,
                };
                let mut split_start = 0;
                let mut pending_break: Option<&Break> = None;
                for (index, child) in docs.iter().enumerate() {
                    if let Doc::Break(separator) = child {
                        inner = self.split(
                            &docs[split_start..index],
                            pending_break,
                            comments,
                            max_width,
                            inner,
                        );
                        pending_break = Some(separator);
                        split_start = index + 1;
                    }
                }
                inner = self.split(
                    &docs[split_start..],
                    pending_break,
                    comments,
                    max_width,
                    inner,
                );
                State {
                    column: inner.column,
                    ..state
                }
            }
            Doc::Token(_) | Doc::Space => State {
                column: state.column + doc.width(),
                ..state
            },
            Doc::Comment { id, text, .. } => {
                let rewritten = comments.rewrite(text, max_width, state.column);
                let last_line = rewritten.rsplit(['\n', '\r']).next().unwrap_or_default();
                let last_line_length = last_line.encode_utf16().count();
                self.comments[*id] = rewritten;
                State {
                    column: state.column + last_line_length,
                    ..state
                }
            }
            Doc::Break(_) => state,
        }
    }

    fn split(
        &mut self,
        split: &[Doc],
        separator: Option<&Break>,
        comments: &dyn Comments,
        max_width: usize,
        mut state: State,
    ) -> State {
        let mut split_width = 0;
        for doc in split {
            split_width = (split_width + doc.width()).min(MAX_LINE_WIDTH);
        }
        if let Some(
            separator @ Break {
                id,
                fill,
                flat,
                plus_indent,
                tag,
            },
        ) = separator
        {
            let break_width = separator.width();
            let should_break = *fill == FillMode::Unified
                || state.must_break
                || state.column + break_width + split_width > max_width;
            if let Some(tag) = tag {
                self.tags[tag.0] = Some(should_break);
            }
            state = match should_break {
                true => {
                    let new_indent = (state.last_indent + plus_indent.eval(self))
                        .max(0)
                        .unsigned_abs() as usize;
                    self.broken[*id] = Some(new_indent);
                    State {
                        column: new_indent,
                        ..state
                    }
                }
                false => {
                    self.broken[*id] = None;
                    State {
                        column: state.column + flat.encode_utf16().count(),
                        ..state
                    }
                }
            };
        }
        let enough_room = state.column + split_width <= max_width;
        state.must_break = false;
        for doc in split {
            state = self.compute(doc, comments, max_width, state);
        }
        state.must_break |= !enough_room;
        state
    }

    pub fn write(&self, doc: &Doc, sink: &mut dyn Sink) {
        match doc {
            Doc::Level { id, docs, flat, .. } => match self.one_line[*id] {
                true => sink.append(flat),
                false => {
                    for child in docs {
                        self.write(child, sink);
                    }
                }
            },
            Doc::Token(text) => sink.append(text),
            Doc::Space => sink.append(" "),
            Doc::Break(Break { id, flat, .. }) => match self.broken[*id] {
                Some(indent) => {
                    sink.append("\n");
                    sink.indent(indent);
                }
                None => sink.append(flat),
            },
            Doc::Comment { id, .. } => sink.append(&self.comments[*id]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::output::TextSink;
    use super::*;

    #[test]
    fn when_compute_with_each_fill_mode_then_returns_the_breaks_gjf_takes() {
        let cases = [
            (FillMode::Unified, 20, "aaa bbb ccc\n"),
            (FillMode::Unified, 8, "aaa\n    bbb\n    ccc\n"),
            (FillMode::Independent, 20, "aaa bbb ccc\n"),
            (FillMode::Independent, 8, "aaa bbb\n    ccc\n"),
            (FillMode::Forced, 20, "aaa\n    bbb\n    ccc\n"),
        ];
        for (fill, max_width, expected) in cases {
            let mut builder = Builder::default();
            let docs = vec![
                Doc::Token("aaa".to_owned()),
                builder.break_doc(fill, " ", Indent::Const(4), None),
                Doc::Token("bbb".to_owned()),
                builder.break_doc(fill, " ", Indent::Const(4), None),
                Doc::Token("ccc".to_owned()),
            ];
            let doc = builder.level(Indent::Const(0), docs);
            let mut layout = Layout::from(&builder);
            let mut sink = TextSink::default();

            layout.compute(
                &doc,
                &|t: &str, _, _| t.to_owned(),
                max_width,
                State {
                    last_indent: 0,
                    indent: 0,
                    column: 0,
                    must_break: false,
                },
            );
            layout.write(&doc, &mut sink);

            assert_eq!(String::from(sink), expected, "{fill:?} {max_width}");
        }
    }

    #[test]
    fn when_compute_with_a_split_too_wide_then_returns_the_next_break_taken_too() {
        let mut builder = Builder::default();
        let docs = vec![
            Doc::Token("a".to_owned()),
            builder.break_doc(FillMode::Independent, " ", Indent::Const(2), None),
            Doc::Token("bbbbbbbbbbbb".to_owned()),
            builder.break_doc(FillMode::Independent, " ", Indent::Const(2), None),
            Doc::Token("c".to_owned()),
        ];
        let doc = builder.level(Indent::Const(0), docs);
        let mut layout = Layout::from(&builder);
        let mut sink = TextSink::default();

        layout.compute(
            &doc,
            &|t: &str, _, _| t.to_owned(),
            10,
            State {
                last_indent: 0,
                indent: 0,
                column: 0,
                must_break: false,
            },
        );
        layout.write(&doc, &mut sink);

        assert_eq!(String::from(sink), "a\n  bbbbbbbbbbbb\n  c\n");
    }

    #[test]
    fn when_compute_with_nested_levels_then_returns_inner_levels_kept_flat_when_they_fit() {
        let mut builder = Builder::default();
        let inner_docs = vec![
            Doc::Token("f(".to_owned()),
            builder.break_doc(FillMode::Unified, "", Indent::Const(4), None),
            Doc::Token("x)".to_owned()),
        ];
        let inner = builder.level(Indent::Const(0), inner_docs);
        let outer_docs = vec![
            Doc::Token("return".to_owned()),
            builder.break_doc(FillMode::Unified, " ", Indent::Const(4), None),
            inner,
            Doc::Space,
            Doc::Token("+ y;".to_owned()),
        ];
        let outer = builder.level(Indent::Const(2), outer_docs);
        let mut layout = Layout::from(&builder);
        let mut sink = TextSink::default();

        layout.compute(
            &outer,
            &|t: &str, _, _| t.to_owned(),
            12,
            State {
                last_indent: 0,
                indent: 0,
                column: 0,
                must_break: false,
            },
        );
        layout.write(&outer, &mut sink);

        assert_eq!(String::from(sink), "return\n      f(x) + y;\n");
    }

    #[test]
    fn when_compute_with_a_tagged_break_then_returns_the_indent_chosen_by_whether_it_broke() {
        let cases = [
            (30, "first second third\n"),
            (12, "first\n    second\n        third\n"),
        ];
        for (max_width, expected) in cases {
            let mut builder = Builder::default();
            let tag = builder.tag();
            let docs = vec![
                Doc::Token("first".to_owned()),
                builder.break_doc(FillMode::Unified, " ", Indent::Const(4), Some(tag)),
                Doc::Token("second".to_owned()),
                builder.break_doc(
                    FillMode::Unified,
                    " ",
                    Indent::If(tag, Box::new(Indent::Const(8)), Box::new(Indent::Const(2))),
                    None,
                ),
                Doc::Token("third".to_owned()),
            ];
            let doc = builder.level(Indent::Const(0), docs);
            let mut layout = Layout::from(&builder);
            let mut sink = TextSink::default();

            layout.compute(
                &doc,
                &|t: &str, _, _| t.to_owned(),
                max_width,
                State {
                    last_indent: 0,
                    indent: 0,
                    column: 0,
                    must_break: false,
                },
            );
            layout.write(&doc, &mut sink);

            assert_eq!(String::from(sink), expected, "{max_width}");
        }
    }

    #[test]
    fn when_compute_with_comments_and_multiline_tokens_then_returns_rewritten_comments_and_forced_breaks()
     {
        let mut builder = Builder::default();
        let comment = builder.comment("//x");
        let docs = vec![
            Doc::Token("a".to_owned()),
            Doc::Space,
            comment,
            builder.break_doc(FillMode::Unified, " ", Indent::Const(0), None),
            Doc::Token("\"\"\"\n  t\n  \"\"\"".to_owned()),
        ];
        let doc = builder.level(Indent::Const(0), docs);
        let mut layout = Layout::from(&builder);
        let mut sink = TextSink::default();

        layout.compute(
            &doc,
            &|t: &str, _, _| t.replacen("//", "// ", 1),
            100,
            State {
                last_indent: 0,
                indent: 0,
                column: 0,
                must_break: false,
            },
        );
        layout.write(&doc, &mut sink);

        assert_eq!(String::from(sink), "a // x\n\"\"\"\n  t\n  \"\"\"\n");
    }

    #[test]
    fn when_compute_with_a_multiline_comment_then_returns_the_column_after_its_last_line() {
        let mut builder = Builder::default();
        let comment = builder.comment("/* a\n   bb */");
        let doc = builder.level(Indent::Const(0), vec![comment]);
        let mut layout = Layout::from(&builder);

        let state = layout.compute(
            &doc,
            &|t: &str, _, _| t.to_owned(),
            4,
            State {
                last_indent: 0,
                indent: 0,
                column: 2,
                must_break: false,
            },
        );

        assert_eq!((state.column, doc.width()), (2 + 8, 4));
    }

    #[test]
    fn when_compute_with_comments_on_one_line_then_returns_them_as_gjf_flattens_them() {
        let cases = [
            ("//x", "// x", 4),
            ("// x", "// x", 4),
            ("/*foo=*/", "/* foo= */", 10),
            ("/* bar... = */", "/* bar...= */", 13),
            ("/* not a name */", "/* not a name */", 16),
            ("/*1=*/", "/*1=*/", 6),
            ("/*null=*/", "/* null= */", 11),
            ("/*_=*/", "/* _= */", 8),
            ("/*int=*/", "/* int= */", 10),
            ("/* a\n b */", "/* a\n b */", 4),
        ];
        for (text, flat, width) in cases {
            let mut builder = Builder::default();
            let comment = builder.comment(text);
            let doc = builder.level(
                Indent::Const(0),
                vec![
                    Doc::Token("f(".to_owned()),
                    comment,
                    Doc::Token("x)".to_owned()),
                ],
            );
            let mut layout = Layout::from(&builder);
            let mut sink = TextSink::default();

            layout.compute(
                &doc,
                &|t: &str, _, _| t.to_owned(),
                100,
                State {
                    last_indent: 0,
                    indent: 0,
                    column: 0,
                    must_break: false,
                },
            );
            layout.write(&doc, &mut sink);

            assert_eq!(
                (String::from(sink), doc.width()),
                (format!("f({flat}x)\n"), width + 4),
                "{text:?}"
            );
        }
    }

    #[test]
    fn when_compute_with_a_large_doc_then_returns_in_time_linear_in_its_size() {
        let mut timings = Vec::new();
        for size in [25_000usize, 100_000] {
            let mut builder = Builder::default();
            let mut docs = Vec::with_capacity(size * 2);
            for _ in 0..size {
                docs.push(Doc::Token("tok".to_owned()));
                docs.push(builder.break_doc(FillMode::Independent, " ", Indent::Const(4), None));
            }
            let doc = builder.level(Indent::Const(0), docs);
            let mut best = std::time::Duration::MAX;
            for _ in 0..3 {
                let mut layout = Layout::from(&builder);
                let mut sink = TextSink::default();
                let start = std::time::Instant::now();
                layout.compute(
                    &doc,
                    &|t: &str, _, _| t.to_owned(),
                    100,
                    State {
                        last_indent: 0,
                        indent: 0,
                        column: 0,
                        must_break: false,
                    },
                );
                layout.write(&doc, &mut sink);
                best = best.min(start.elapsed());
            }
            timings.push(best.as_secs_f64());
        }

        let ratio = timings[1] / timings[0].max(f64::EPSILON);

        assert!(ratio < 10.0, "4x the doc took {ratio:.1}x the time");
    }
}
