use crate::lexer;
use crate::lexer::token::{Token, TokenKind};

pub const MAX_LINE_WIDTH: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakMode {
    Unified,
    Fill,
    Forced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakTag(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indent {
    Const(i32),
    If(BreakTag, Box<Indent>, Box<Indent>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMark {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Break {
    pub mode: BreakMode,
    pub flat: String,
    pub plus_indent: Indent,
    pub tag: Option<BreakTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    plus_indent: Indent,
    docs: Vec<Doc>,
    width: usize,
    flat: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub preferred: Group,
    pub prefix_width: usize,
    pub fallback: Group,
    pub fallback_if_flat_on_next_line: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    Text(String),
    Lines(Vec<String>),
    Comment {
        text: String,
        flat: String,
        width: usize,
    },
    Space,
    Header(HeaderMark),
    Break(Break),
    Group(Group),
    Choice(Box<Choice>),
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

fn total_width(docs: &[Doc]) -> usize {
    let mut width = 0;
    for doc in docs {
        width = (width + doc.width()).min(MAX_LINE_WIDTH);
        if width == MAX_LINE_WIDTH {
            return width;
        }
    }
    width
}

fn first_break(text: &str) -> Option<usize> {
    let index = text.find(['\n', '\r'])?;
    let before: usize = text[..index].chars().map(char::len_utf16).sum();
    match text[index..].starts_with("\r\n") {
        true => Some(before + 2),
        false => Some(before + 1),
    }
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
        match self.mode {
            BreakMode::Forced => MAX_LINE_WIDTH,
            BreakMode::Unified | BreakMode::Fill => self.flat.chars().map(char::len_utf16).sum(),
        }
    }
}

impl Group {
    pub fn new(plus_indent: Indent, docs: Vec<Doc>) -> Group {
        let width = total_width(&docs);
        let flat: String = docs.iter().map(Doc::flat).collect();
        Group {
            plus_indent,
            docs,
            width,
            flat,
        }
    }
}

impl Doc {
    pub fn comment(text: &str) -> Doc {
        let parameter = parameter_comment(text);
        let flat = match (
            text.starts_with("//") && !text.starts_with("// "),
            parameter,
        ) {
            (true, _) => format!("// {}", &text[2..]),
            (false, Some(reformatted)) => reformatted,
            (false, None) => text.to_owned(),
        };
        let width = match first_break(text) {
            Some(second_line) => second_line,
            None => flat.chars().map(char::len_utf16).sum(),
        };
        Doc::Comment {
            text: text.to_owned(),
            flat,
            width,
        }
    }

    pub fn width(&self) -> usize {
        match self {
            Doc::Text(text) => match first_break(text) {
                Some(_) => MAX_LINE_WIDTH,
                None => text.chars().map(char::len_utf16).sum(),
            },
            Doc::Lines(_) => MAX_LINE_WIDTH,
            Doc::Comment { width, .. } => *width,
            Doc::Space => 1,
            Doc::Header(_) => 0,
            Doc::Break(separator) => separator.width(),
            Doc::Group(group) => group.width,
            Doc::Choice(choice) => choice.fallback.width,
        }
    }

    pub fn flat(&self) -> &str {
        match self {
            Doc::Text(flat) | Doc::Comment { flat, .. } => flat,
            Doc::Break(Break { flat, .. }) | Doc::Group(Group { flat, .. }) => flat,
            Doc::Choice(choice) => &choice.fallback.flat,
            Doc::Space => " ",
            Doc::Lines(_) | Doc::Header(_) => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Append(String),
    Newline(usize),
    IndentTo(usize),
}

#[derive(Debug, Clone, Copy)]
struct State {
    indent: i32,
    column: usize,
}

enum Ended {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy)]
enum Trial {
    LineEnded(usize),
    Fit(usize),
}

struct Solver<'a> {
    max_width: usize,
    comments: &'a dyn Comments,
    actions: Vec<Action>,
    tags: Vec<bool>,
    first_line_ended: bool,
    first_line_end_column: usize,
    line_indent: usize,
    header_indents: Vec<usize>,
    trial_tags: Vec<Vec<(BreakTag, bool)>>,
}

impl Indent {
    fn eval(&self, tags: &[bool]) -> i32 {
        match self {
            Indent::Const(n) => *n,
            Indent::If(tag, then, otherwise) => match tags.get(tag.0) {
                Some(true) => then.eval(tags),
                Some(false) | None => otherwise.eval(tags),
            },
        }
    }
}

impl Solver<'_> {
    fn group(&mut self, group: &Group, state: State) -> State {
        if state.column + group.width <= self.max_width {
            self.actions.push(Action::Append(group.flat.clone()));
            return State {
                column: state.column + group.width,
                ..state
            };
        }
        let indent = state.indent + group.plus_indent.eval(&self.tags);
        let mut inner = State {
            indent,
            column: state.column,
        };
        let mut before: Option<&Break> = None;
        let mut previous_fits = true;
        let mut start = 0;
        for (index, doc) in group.docs.iter().enumerate() {
            if let Doc::Break(separator) = doc {
                let (next, fits) =
                    self.segment(before, &group.docs[start..index], previous_fits, inner);
                inner = next;
                if self.first_line_ended {
                    return State {
                        column: inner.column,
                        ..state
                    };
                }
                previous_fits = fits;
                before = Some(separator);
                start = index + 1;
            }
        }
        let (end, _) = self.segment(before, &group.docs[start..], previous_fits, inner);
        State {
            column: end.column,
            ..state
        }
    }

    fn segment(
        &mut self,
        before: Option<&Break>,
        segment: &[Doc],
        previous_fits: bool,
        mut state: State,
    ) -> (State, bool) {
        let width = total_width(segment);
        if let Some(separator) = before {
            let taken = match separator.mode {
                BreakMode::Unified | BreakMode::Forced => true,
                BreakMode::Fill => {
                    !previous_fits || state.column + separator.width() + width > self.max_width
                }
            };
            state = self.break_op(separator, taken, state);
            if self.first_line_ended {
                return (state, true);
            }
        }
        let fits = state.column + width <= self.max_width;
        for doc in segment {
            state = self.leaf(doc, state);
            if self.first_line_ended {
                return (state, fits);
            }
        }
        (state, fits)
    }

    fn break_op(&mut self, separator: &Break, taken: bool, state: State) -> State {
        if let Some(tag) = separator.tag {
            if self.tags.len() <= tag.0 {
                self.tags.resize(tag.0 + 1, false);
            }
            if let Some(saved) = self.trial_tags.last_mut() {
                saved.push((tag, self.tags[tag.0]));
            }
            self.tags[tag.0] = taken;
        }
        match taken {
            true => {
                if let Ended::Yes = self.end_line(state.column) {
                    return state;
                }
                let indent = (state.indent + separator.plus_indent.eval(&self.tags))
                    .max(0)
                    .unsigned_abs() as usize;
                self.line_indent = indent;
                self.actions.push(Action::Newline(1));
                self.actions.push(Action::IndentTo(indent));
                State {
                    column: indent,
                    ..state
                }
            }
            false => {
                self.actions.push(Action::Append(separator.flat.clone()));
                State {
                    column: state.column + separator.width(),
                    ..state
                }
            }
        }
    }

    fn leaf(&mut self, doc: &Doc, state: State) -> State {
        match doc {
            Doc::Group(group) => self.group(group, state),
            Doc::Choice(choice) => self.choice(choice, state),
            Doc::Text(text) => {
                if let Some(second_line) = first_break(text) {
                    let _ = self.end_line(state.column + second_line);
                }
                self.actions.push(Action::Append(text.clone()));
                State {
                    column: state.column + doc.width(),
                    ..state
                }
            }
            Doc::Lines(lines) => {
                let first = lines.first().map_or("", String::as_str);
                let first_width = first.chars().map(char::len_utf16).sum::<usize>();
                if let Ended::Yes = self.end_line(state.column + first_width) {
                    return state;
                }
                let indent = self.line_indent;
                self.actions.push(Action::Append(first.to_owned()));
                let mut newlines = 0;
                for line in lines.iter().skip(1) {
                    newlines += 1;
                    if line.is_empty() {
                        continue;
                    }
                    self.actions.push(Action::Newline(newlines));
                    newlines = 0;
                    if indent > 0 {
                        self.actions.push(Action::IndentTo(indent));
                    }
                    self.actions.push(Action::Append(line.clone()));
                }
                if newlines > 0 {
                    self.actions.push(Action::Newline(newlines));
                }
                State {
                    column: state.column + MAX_LINE_WIDTH,
                    ..state
                }
            }
            Doc::Space => {
                self.actions.push(Action::Append(" ".to_owned()));
                State {
                    column: state.column + 1,
                    ..state
                }
            }
            Doc::Header(mark) => {
                match mark {
                    HeaderMark::Start => self.header_indents.push(self.line_indent),
                    HeaderMark::End => {
                        self.header_indents.pop();
                    }
                }
                state
            }
            Doc::Comment { text, .. } => {
                let rewritten = self.comments.rewrite(text, self.max_width, state.column);
                if let Some(second_line) = first_break(&rewritten) {
                    let _ = self.end_line(state.column + second_line);
                }
                let last_line = rewritten.rsplit(['\n', '\r']).next().unwrap_or_default();
                let column = state.column + last_line.chars().map(char::len_utf16).sum::<usize>();
                self.actions.push(Action::Append(rewritten));
                State { column, ..state }
            }
            Doc::Break(_) => state,
        }
    }

    fn choice(&mut self, choice: &Choice, state: State) -> State {
        let width = choice.fallback.width;
        if state.column + width <= self.max_width {
            self.actions
                .push(Action::Append(choice.fallback.flat.clone()));
            return State {
                column: state.column + width,
                ..state
            };
        }
        if choice.fallback_if_flat_on_next_line {
            let docs = &choice.fallback.docs;
            let mut breaks = docs.iter().enumerate().filter_map(|(i, d)| match d {
                Doc::Break(separator) => Some((i, separator)),
                _ => None,
            });
            if let (Some((index, separator)), None) = (breaks.next(), breaks.next()) {
                let indent = (state.indent
                    + choice.fallback.plus_indent.eval(&self.tags)
                    + separator.plus_indent.eval(&self.tags))
                .max(0)
                .unsigned_abs() as usize;
                let prefix_fits = state.column + total_width(&docs[..index]) <= self.max_width;
                if prefix_fits && indent + total_width(&docs[index + 1..]) <= self.max_width {
                    return self.group(&choice.fallback, state);
                }
            }
        }
        let preferred_state = State {
            indent: i32::try_from(self.line_indent).unwrap_or(i32::MAX),
            column: state.column,
        };
        let prefix_end = state.column + choice.prefix_width;
        let on_header_line = self.header_indents.last() == Some(&self.line_indent);
        if prefix_end <= self.max_width && !on_header_line {
            let trial = self.first_line_end(&choice.preferred, preferred_state);
            let (Trial::LineEnded(column) | Trial::Fit(column)) = trial;
            if prefix_end <= column && column <= self.max_width {
                if let Trial::LineEnded(_) = trial
                    && let Ended::Yes = self.end_line(column)
                {
                    return state;
                }
                let end = self.group(&choice.preferred, preferred_state);
                return State {
                    column: end.column,
                    ..state
                };
            }
        }
        self.group(&choice.fallback, state)
    }

    fn first_line_end(&mut self, group: &Group, state: State) -> Trial {
        let mark = self.actions.len();
        let line_indent = self.line_indent;
        let header_indents = self.header_indents.clone();
        self.trial_tags.push(Vec::new());
        let end = self.group(group, state);
        let trial = match self.first_line_ended {
            true => Trial::LineEnded(self.first_line_end_column),
            false => Trial::Fit(end.column),
        };
        self.first_line_ended = false;
        self.line_indent = line_indent;
        self.header_indents = header_indents;
        self.actions.truncate(mark);
        for (tag, previous) in self.trial_tags.pop().unwrap_or_default().into_iter().rev() {
            self.tags[tag.0] = previous;
        }
        trial
    }

    fn end_line(&mut self, column: usize) -> Ended {
        match self.trial_tags.last() {
            Some(_) => {
                self.first_line_ended = true;
                self.first_line_end_column = column;
                Ended::Yes
            }
            None => Ended::No,
        }
    }
}

pub fn format(doc: &Doc, max_width: usize, comments: &dyn Comments, sink: &mut dyn Sink) {
    let root = match doc {
        Doc::Group(group) => group.clone(),
        other => Group::new(Indent::Const(0), vec![other.clone()]),
    };
    let mut solver = Solver {
        max_width,
        comments,
        actions: Vec::new(),
        tags: Vec::new(),
        first_line_ended: false,
        first_line_end_column: 0,
        line_indent: 0,
        header_indents: Vec::new(),
        trial_tags: Vec::new(),
    };
    solver.group(
        &root,
        State {
            indent: 0,
            column: 0,
        },
    );
    for action in solver.actions {
        match action {
            Action::Append(text) => sink.append(&text),
            Action::Newline(count) => sink.append(&"\n".repeat(count)),
            Action::IndentTo(spaces) => sink.indent(spaces),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::output::TextSink;
    use super::*;

    #[test]
    fn when_format_with_layout_cases_then_returns_what_gjf_layout_solver_returns() {
        let text = |s: &str| Doc::Text(s.to_owned());
        let brk = |mode: BreakMode, flat: &str, indent: i32| {
            Doc::Break(Break {
                mode,
                flat: flat.to_owned(),
                plus_indent: Indent::Const(indent),
                tag: None,
            })
        };
        let group = |indent: i32, docs: Vec<Doc>| Group::new(Indent::Const(indent), docs);
        let modes = |mode: BreakMode| {
            Doc::Group(group(
                0,
                vec![
                    text("aaa"),
                    brk(mode, " ", 4),
                    text("bbb"),
                    brk(mode, " ", 4),
                    text("ccc"),
                ],
            ))
        };
        let tagged = || {
            let tag = BreakTag(0);
            Doc::Group(group(
                0,
                vec![
                    text("first"),
                    Doc::Break(Break {
                        mode: BreakMode::Unified,
                        flat: " ".to_owned(),
                        plus_indent: Indent::Const(4),
                        tag: Some(tag),
                    }),
                    text("second"),
                    Doc::Break(Break {
                        mode: BreakMode::Unified,
                        flat: " ".to_owned(),
                        plus_indent: Indent::If(
                            tag,
                            Box::new(Indent::Const(8)),
                            Box::new(Indent::Const(2)),
                        ),
                        tag: None,
                    }),
                    text("third"),
                ],
            ))
        };
        let body = || {
            group(
                0,
                vec![
                    text("{"),
                    brk(BreakMode::Forced, "", 2),
                    text("work();"),
                    brk(BreakMode::Forced, "", 0),
                    text("}"),
                ],
            )
        };
        let hug = || {
            Doc::Choice(Box::new(Choice {
                preferred: group(
                    0,
                    vec![text("call(x ->"), Doc::Space, Doc::Group(body()), text(")")],
                ),
                prefix_width: 9,
                fallback: group(
                    0,
                    vec![
                        text("call("),
                        brk(BreakMode::Unified, "", 4),
                        Doc::Group(group(0, vec![text("x ->"), Doc::Space, Doc::Group(body())])),
                        text(")"),
                    ],
                ),
                fallback_if_flat_on_next_line: false,
            }))
        };
        let next_line = || {
            Doc::Choice(Box::new(Choice {
                preferred: group(
                    0,
                    vec![
                        text("f(x ->"),
                        brk(BreakMode::Unified, " ", 4),
                        text("g(x))"),
                    ],
                ),
                prefix_width: 6,
                fallback: group(
                    0,
                    vec![
                        text("f("),
                        brk(BreakMode::Unified, "", 4),
                        text("x -> g(x))"),
                    ],
                ),
                fallback_if_flat_on_next_line: true,
            }))
        };
        let hugged = || {
            group(
                0,
                vec![
                    text("{"),
                    brk(BreakMode::Forced, "", 2),
                    text("a();"),
                    brk(BreakMode::Forced, "", 0),
                    text("}"),
                ],
            )
        };
        let header = Doc::Group(group(
            0,
            vec![
                Doc::Header(HeaderMark::Start),
                text("if ("),
                Doc::Choice(Box::new(Choice {
                    preferred: group(
                        0,
                        vec![
                            text("run(() ->"),
                            Doc::Space,
                            Doc::Group(hugged()),
                            text(")"),
                        ],
                    ),
                    prefix_width: 9,
                    fallback: group(
                        0,
                        vec![
                            text("run("),
                            brk(BreakMode::Unified, "", 4),
                            Doc::Group(group(
                                0,
                                vec![text("() ->"), Doc::Space, Doc::Group(hugged())],
                            )),
                            text(")"),
                        ],
                    ),
                    fallback_if_flat_on_next_line: false,
                })),
                text(")"),
                Doc::Header(HeaderMark::End),
                text(" {}"),
            ],
        ));
        let lines = Doc::Group(group(
            2,
            vec![
                text("s ="),
                brk(BreakMode::Forced, " ", 4),
                Doc::Lines(vec![
                    "\"\"\"".into(),
                    "  a".into(),
                    "".into(),
                    "b".into(),
                    "\"\"\";".into(),
                ]),
            ],
        ));
        let fallback_next_line = Doc::Choice(Box::new(Choice {
            preferred: group(
                0,
                vec![
                    text("callSomething(x ->"),
                    brk(BreakMode::Unified, " ", 4),
                    text("gg(xx))"),
                ],
            ),
            prefix_width: 18,
            fallback: group(
                0,
                vec![
                    text("callSomething("),
                    brk(BreakMode::Unified, "", 4),
                    text("x -> gg(xx))"),
                ],
            ),
            fallback_if_flat_on_next_line: true,
        }));
        let nested_choice = || {
            let tag = BreakTag(0);
            let inner = || {
                group(
                    0,
                    vec![
                        text("{"),
                        brk(BreakMode::Forced, "", 2),
                        text("b();"),
                        brk(BreakMode::Forced, "", 0),
                        text("}"),
                    ],
                )
            };
            let outer_body = || {
                group(
                    0,
                    vec![
                        text("{"),
                        Doc::Break(Break {
                            mode: BreakMode::Unified,
                            flat: String::new(),
                            plus_indent: Indent::Const(2),
                            tag: Some(tag),
                        }),
                        Doc::Choice(Box::new(Choice {
                            preferred: group(
                                0,
                                vec![text("g(y ->"), Doc::Space, Doc::Group(inner()), text(")")],
                            ),
                            prefix_width: 6,
                            fallback: group(
                                0,
                                vec![
                                    text("g("),
                                    brk(BreakMode::Unified, "", 4),
                                    Doc::Group(group(
                                        0,
                                        vec![text("y ->"), Doc::Space, Doc::Group(inner())],
                                    )),
                                    text(")"),
                                ],
                            ),
                            fallback_if_flat_on_next_line: false,
                        })),
                        Doc::Break(Break {
                            mode: BreakMode::Unified,
                            flat: String::new(),
                            plus_indent: Indent::If(
                                tag,
                                Box::new(Indent::Const(0)),
                                Box::new(Indent::Const(6)),
                            ),
                            tag: None,
                        }),
                        text("}"),
                    ],
                )
            };
            Doc::Choice(Box::new(Choice {
                preferred: group(
                    0,
                    vec![
                        text("f(x ->"),
                        Doc::Space,
                        Doc::Group(outer_body()),
                        text(")"),
                    ],
                ),
                prefix_width: 6,
                fallback: group(
                    0,
                    vec![
                        text("f("),
                        brk(BreakMode::Unified, "", 4),
                        Doc::Group(group(
                            0,
                            vec![text("x ->"), Doc::Space, Doc::Group(outer_body())],
                        )),
                        text(")"),
                    ],
                ),
                fallback_if_flat_on_next_line: false,
            }))
        };
        let cases = [
            (
                "modes/UNIFIED/20",
                modes(BreakMode::Unified),
                20,
                "aaa bbb ccc",
            ),
            (
                "modes/UNIFIED/8",
                modes(BreakMode::Unified),
                8,
                "aaa\n    bbb\n    ccc",
            ),
            ("modes/FILL/20", modes(BreakMode::Fill), 20, "aaa bbb ccc"),
            (
                "modes/FILL/8",
                modes(BreakMode::Fill),
                8,
                "aaa bbb\n    ccc",
            ),
            (
                "modes/FORCED/20",
                modes(BreakMode::Forced),
                20,
                "aaa\n    bbb\n    ccc",
            ),
            (
                "modes/FORCED/8",
                modes(BreakMode::Forced),
                8,
                "aaa\n    bbb\n    ccc",
            ),
            (
                "previous-too-wide",
                Doc::Group(group(
                    0,
                    vec![
                        text("a"),
                        brk(BreakMode::Fill, " ", 2),
                        text("bbbbbbbbbbbb"),
                        brk(BreakMode::Fill, " ", 2),
                        text("c"),
                    ],
                )),
                10,
                "a\n  bbbbbbbbbbbb\n  c",
            ),
            (
                "nested",
                Doc::Group(group(
                    2,
                    vec![
                        text("return"),
                        brk(BreakMode::Unified, " ", 4),
                        Doc::Group(group(
                            0,
                            vec![text("f("), brk(BreakMode::Unified, "", 4), text("x)")],
                        )),
                        Doc::Space,
                        text("+ y;"),
                    ],
                )),
                12,
                "return\n      f(x) + y;",
            ),
            ("tagged/30", tagged(), 30, "first second third"),
            (
                "tagged/12",
                tagged(),
                12,
                "first\n    second\n        third",
            ),
            ("choice/40", hug(), 40, "call(x -> {\n  work();\n})"),
            ("choice/20", hug(), 20, "call(x -> {\n  work();\n})"),
            ("choice/12", hug(), 12, "call(x -> {\n  work();\n})"),
            ("choice-next-line/40", next_line(), 40, "f(x -> g(x))"),
            ("choice-next-line/16", next_line(), 16, "f(x -> g(x))"),
            (
                "choice-next-line-fallback",
                fallback_next_line,
                20,
                "callSomething(\n    x -> gg(xx))",
            ),
            (
                "nested-choice/24",
                nested_choice(),
                24,
                "f(x -> {\n  g(y -> {\n    b();\n  })\n})",
            ),
            (
                "nested-choice/14",
                nested_choice(),
                14,
                "f(x -> {\n  g(y -> {\n    b();\n  })\n})",
            ),
            (
                "header",
                header,
                30,
                "if (run(\n    () -> {\n  a();\n})) {}",
            ),
            (
                "lines",
                lines,
                20,
                "s =\n      \"\"\"\n        a\n\n      b\n      \"\"\";",
            ),
            (
                "multiline-text",
                Doc::Group(group(
                    0,
                    vec![
                        text("x"),
                        Doc::Space,
                        text("/* a\n b */"),
                        Doc::Space,
                        text("y"),
                    ],
                )),
                40,
                "x /* a\n b */ y",
            ),
        ];
        for (name, doc, max_width, expected) in cases {
            let mut sink = TextSink::default();

            format(&doc, max_width, &|t: &str, _, _| t.to_owned(), &mut sink);

            assert_eq!(String::from(sink), format!("{expected}\n"), "{name}");
        }
    }

    #[test]
    fn when_comment_with_each_kind_then_returns_the_flat_text_and_width_gjf_measures() {
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
            ("/* a\n b */", "/* a\n b */", 5),
            ("/* a\r\n b */", "/* a\r\n b */", 6),
        ];
        for (text, flat, width) in cases {
            let comment = Doc::comment(text);

            assert_eq!((comment.flat(), comment.width()), (flat, width), "{text:?}");
        }
    }

    #[test]
    fn when_format_with_comments_then_returns_them_rewritten_at_their_column() {
        let doc = Doc::Group(Group::new(
            Indent::Const(0),
            vec![
                Doc::Text("f(".to_owned()),
                Doc::comment("/*x*/"),
                Doc::Break(Break {
                    mode: BreakMode::Unified,
                    flat: " ".to_owned(),
                    plus_indent: Indent::Const(4),
                    tag: None,
                }),
                Doc::Text("a_long_argument)".to_owned()),
            ],
        ));
        let mut sink = TextSink::default();

        format(
            &doc,
            10,
            &|t: &str, _, column: usize| format!("{t}@{column}"),
            &mut sink,
        );

        assert_eq!(String::from(sink), "f(/*x*/@2\n    a_long_argument)\n");
    }

    #[test]
    fn when_format_with_a_large_doc_then_returns_in_time_linear_in_its_size() {
        let mut timings = Vec::new();
        for size in [25_000usize, 100_000] {
            let mut docs = Vec::with_capacity(size * 2);
            for _ in 0..size {
                docs.push(Doc::Text("tok".to_owned()));
                docs.push(Doc::Break(Break {
                    mode: BreakMode::Fill,
                    flat: " ".to_owned(),
                    plus_indent: Indent::Const(4),
                    tag: None,
                }));
            }
            let doc = Doc::Group(Group::new(Indent::Const(0), docs));
            let mut best = std::time::Duration::MAX;
            for _ in 0..3 {
                let mut sink = TextSink::default();
                let start = std::time::Instant::now();
                format(&doc, 100, &|t: &str, _, _| t.to_owned(), &mut sink);
                best = best.min(start.elapsed());
            }
            timings.push(best.as_secs_f64());
        }

        let ratio = timings[1] / timings[0].max(f64::EPSILON);

        assert!(ratio < 10.0, "4x the doc took {ratio:.1}x the time");
    }
}
