use super::builder::Unformatted;
use super::input;
use crate::lexer::identifier::{self, CharClass};
use crate::lexer::token::{Punct, Token, TokenClass, TokenKind};
use crate::lexer::{self};
use crate::parser::Unit;
use crate::syntax::{Element, Node, NodeKind};

#[derive(Debug, Clone, Copy)]
enum Request {
    Semicolon(usize),
    Range(usize, usize),
}

struct Context<'a> {
    length: usize,
    semicolons: &'a [usize],
    comments: &'a [usize],
}

fn span(node: &Node) -> Option<(usize, usize)> {
    let tokens = node.tokens();
    let mut significant = tokens
        .iter()
        .filter(|token| matches!(token.kind.class(), TokenClass::Significant));
    let first = significant.next()?;
    let last = significant.next_back().unwrap_or(first);
    Some((first.start, last.end))
}

fn semicolons_outside<'n>(
    context: &Context,
    (start, end): (usize, usize),
    members: impl Iterator<Item = &'n Node>,
    separator: Option<(usize, usize)>,
) -> Vec<Request> {
    let spans: Vec<(usize, usize)> = members
        .filter(|member| member.kind != NodeKind::EmptyDecl)
        .filter_map(span)
        .chain(separator)
        .collect();
    context
        .semicolons
        .iter()
        .filter(|&&semicolon| {
            (start..end).contains(&semicolon)
                && !spans
                    .iter()
                    .any(|&(low, high)| (low..high).contains(&semicolon))
        })
        .map(|&semicolon| Request::Semicolon(semicolon))
        .collect()
}

fn body_semicolons(context: &Context, body: &Node, owner: &Node) -> Vec<Request> {
    let (Some(body_span), Some((owner_start, _))) = (span(body), span(owner)) else {
        return Vec::new();
    };
    let members: Vec<(NodeKind, (usize, usize))> = body
        .children
        .iter()
        .filter_map(|child| match child {
            Element::Node(member) if member.kind != NodeKind::EmptyDecl => {
                span(member).map(|member_span| (member.kind, member_span))
            }
            Element::Node(_) | Element::Token(_) => None,
        })
        .collect();
    let last_constant_end = members
        .iter()
        .filter(|(kind, _)| *kind == NodeKind::EnumConstant)
        .map(|(_, (_, high))| *high)
        .fold(owner_start, usize::max);
    let first_member_start = members
        .iter()
        .filter(|(kind, _)| *kind != NodeKind::EnumConstant)
        .map(|(_, (low, _))| *low)
        .fold(body_span.1, usize::min);
    let separator = context
        .semicolons
        .iter()
        .find(|semicolon| (last_constant_end..first_member_start).contains(semicolon))
        .filter(|_| {
            matches!(owner.kind, NodeKind::EnumDecl | NodeKind::EnumConstant) && !members.is_empty()
        })
        .map(|&semicolon| (semicolon, semicolon + 1));
    let nodes = body.children.iter().filter_map(|child| match child {
        Element::Node(member) => Some(member),
        Element::Token(_) => None,
    });
    semicolons_outside(context, body_span, nodes, separator)
}

fn requests(context: &Context, node: &Node) -> Vec<Request> {
    let nodes: Vec<&Node> = node
        .children
        .iter()
        .filter_map(|child| match child {
            Element::Node(child) => Some(child),
            Element::Token(_) => None,
        })
        .collect();
    let body = nodes
        .iter()
        .copied()
        .into_iter()
        .find(|child| matches!(child.kind, NodeKind::ClassBody | NodeKind::EnumBody));
    let before = match (node.kind, body) {
        (NodeKind::CompilationUnit, _) => {
            semicolons_outside(context, (0, context.length), nodes.iter().copied(), None)
        }
        (
            NodeKind::ClassDecl
            | NodeKind::InterfaceDecl
            | NodeKind::EnumDecl
            | NodeKind::RecordDecl
            | NodeKind::AnnotationTypeDecl,
            Some(body),
        ) => body_semicolons(context, body, node),
        (_, _) => Vec::new(),
    };
    let children = nodes.iter().copied().flat_map(|child| {
        let anonymous = match (node.kind, child.kind) {
            (NodeKind::NewClass | NodeKind::EnumConstant, NodeKind::ClassBody) => {
                body_semicolons(context, child, node)
            }
            (_, _) => Vec::new(),
        };
        let empty = match (node.kind, child.kind, span(child)) {
            (
                NodeKind::Block | NodeKind::SwitchGroup,
                NodeKind::EmptyStmt,
                Some((semicolon, _)),
            ) => {
                vec![Request::Semicolon(semicolon)]
            }
            (_, _, _) => Vec::new(),
        };
        anonymous
            .into_iter()
            .chain(empty)
            .chain(requests(context, child))
    });
    let commented = |start: usize, end: usize| {
        context
            .comments
            .iter()
            .any(|comment| (start..end).contains(comment))
    };
    let after = match node.kind {
        NodeKind::Annotation => {
            let name_end = nodes
                .iter()
                .copied()
                .into_iter()
                .find(|child| child.kind == NodeKind::QualifiedName)
                .and_then(span)
                .map(|(_, end)| end);
            let arguments = nodes
                .iter()
                .copied()
                .into_iter()
                .find(|child| child.kind == NodeKind::AnnotationArguments)
                .filter(|arguments| {
                    arguments
                        .children
                        .iter()
                        .all(|child| matches!(child, Element::Token(_)))
                })
                .and_then(span);
            match (name_end, arguments) {
                (Some(start), Some((_, end))) if !commented(start, end) => {
                    vec![Request::Range(start, end)]
                }
                (_, _) => Vec::new(),
            }
        }
        NodeKind::Lambda => {
            let significant: Vec<Token> = node
                .tokens()
                .into_iter()
                .filter(|token| matches!(token.kind.class(), TokenClass::Significant))
                .take(3)
                .collect();
            match significant.as_slice() {
                [open, _, close]
                    if open.kind == TokenKind::Punct(Punct::LParen)
                        && close.kind == TokenKind::Punct(Punct::RParen)
                        && !commented(open.start, close.end) =>
                {
                    vec![
                        Request::Range(open.start, open.end),
                        Request::Range(close.start, close.end),
                    ]
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    before.into_iter().chain(children).chain(after).collect()
}

fn merge(deletions: Vec<(usize, usize)>, range: (usize, usize)) -> Vec<(usize, usize)> {
    let sorted = {
        let mut sorted = deletions;
        sorted.push(range);
        sorted.sort_unstable();
        sorted
    };
    sorted.into_iter().fold(
        Vec::new(),
        |merged: Vec<(usize, usize)>, (low, high)| match merged.split_last() {
            Some((&(last_low, last_high), rest)) if low <= last_high => rest
                .iter()
                .copied()
                .chain([(last_low, last_high.max(high))])
                .collect(),
            Some(_) | None => merged.into_iter().chain([(low, high)]).collect(),
        },
    )
}

fn semicolon_range(source: &str, deletions: &[(usize, usize)], semicolon: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let horizontal = |byte: &&u8| matches!(byte, b' ' | b'\t');
    let start = semicolon
        - bytes[..semicolon]
            .iter()
            .rev()
            .take_while(horizontal)
            .count();
    let line_begin = start
        - bytes[..start]
            .iter()
            .rev()
            .take_while(|byte| !matches!(byte, b'\n' | b'\r'))
            .count();
    let end = semicolon + 1 + bytes[semicolon + 1..].iter().take_while(horizontal).count();
    let nothing_before = start == line_begin
        || deletions
            .iter()
            .any(|&(low, high)| low <= line_begin && start <= high);
    let end = match (nothing_before, bytes.get(end), bytes.get(end + 1)) {
        (true, Some(b'\r'), Some(b'\n')) => end + 2,
        (true, Some(b'\r' | b'\n'), _) => end + 1,
        (_, _, _) => end,
    };
    (start, end)
}

pub fn ast_cleanups(source: &str, unit: &Unit) -> String {
    let tokens = unit.tree.tokens();
    let positions = |kinds: &[TokenKind]| -> Vec<usize> {
        tokens
            .iter()
            .filter(|token| kinds.contains(&token.kind))
            .map(|token| token.start)
            .collect()
    };
    let semicolons = positions(&[TokenKind::Punct(Punct::Semicolon)]);
    let comments = positions(&[
        TokenKind::LineComment,
        TokenKind::BlockComment,
        TokenKind::JavadocComment,
    ]);
    let context = Context {
        length: source.len(),
        semicolons: &semicolons,
        comments: &comments,
    };
    let deletions =
        requests(&context, &unit.tree)
            .into_iter()
            .fold(Vec::new(), |deletions, request| {
                let range = match request {
                    Request::Semicolon(semicolon) => semicolon_range(source, &deletions, semicolon),
                    Request::Range(start, end) => (start, end),
                };
                merge(deletions, range)
            });
    let identifier_part = |c: Option<char>| {
        c.filter(|c| c.len_utf16() == 1)
            .is_some_and(|c| identifier::class(c) != CharClass::Other)
    };
    let (cleaned, cursor) =
        deletions
            .iter()
            .fold((String::new(), 0), |(cleaned, cursor), &(start, end)| {
                let separator = match (
                    identifier_part(source[..start].chars().next_back()),
                    identifier_part(source[end..].chars().next()),
                ) {
                    (true, true) => " ",
                    (_, _) => "",
                };
                (cleaned + &source[cursor..start] + separator, end)
            });
    cleaned + &source[cursor..]
}

pub fn reorder_modifiers(source: &str) -> Result<String, Unformatted> {
    let lexed = lexer::scan(source).map_err(|error| Unformatted::Rejected {
        offset: error.offset,
        message: error.kind.to_string(),
    })?;
    let input = input::build(source, &lexed.tokens);
    let tokens = &input.tokens;
    let text = |index: usize| {
        let tok = &tokens[index].tok;
        lexed
            .text
            .decoded(tok.position, tok.position + tok.text.len())
    };
    let order = [
        "public",
        "protected",
        "private",
        "abstract",
        "default",
        "static",
        "sealed",
        "non-sealed",
        "final",
        "transient",
        "volatile",
        "synchronized",
        "native",
        "strictfp",
    ];
    let rank = |name: &str| order.iter().position(|modifier| *modifier == name);
    let modifier_at = |index: usize| -> (Option<(usize, usize, usize)>, usize) {
        match (text(index).as_str(), index + 2 < tokens.len()) {
            ("non", true) if text(index + 1) == "-" => {
                let modifier = match text(index + 2).as_str() {
                    "sealed" => rank("non-sealed").map(|rank| (rank, index, index + 2)),
                    _ => None,
                };
                (modifier, index + 3)
            }
            ("non", _) => (None, (index + 2).min(tokens.len())),
            (name, _) => (rank(name).map(|rank| (rank, index, index)), index + 1),
        }
    };
    let modifiers: Vec<Option<(usize, usize, usize)>> =
        std::iter::successors(Some(modifier_at(0)), |&(_, next)| {
            (next < tokens.len()).then(|| modifier_at(next))
        })
        .map(|(modifier, _)| modifier)
        .collect();
    let trivia =
        |toks: &[input::Tok]| -> String { toks.iter().map(|tok| tok.text.as_str()).collect() };
    let replacements: Vec<(usize, usize, String)> = modifiers
        .split(Option::is_none)
        .map(|run| run.iter().flatten().copied().collect::<Vec<_>>())
        .filter(|run| run.windows(2).any(|pair| pair[0].0 > pair[1].0))
        .map(|run| {
            let sorted = {
                let mut sorted = run.clone();
                sorted.sort_by_key(|modifier| modifier.0);
                sorted
            };
            let replacement: String = sorted
                .iter()
                .zip(&run)
                .enumerate()
                .map(|(position, (modifier, &(_, first, last)))| {
                    let before = match position {
                        0 => String::new(),
                        _ => trivia(&tokens[first].before),
                    };
                    let after = match position + 1 == sorted.len() {
                        true => String::new(),
                        false => trivia(&tokens[last].after),
                    };
                    before + order[modifier.0] + &after
                })
                .collect();
            let begin = tokens[run[0].1].tok.position;
            let last = &tokens[run[run.len() - 1].2].tok;
            (begin, last.position + last.text.len(), replacement)
        })
        .collect();
    Ok(replacements
        .into_iter()
        .rev()
        .fold(source.to_owned(), |text, (start, end, replacement)| {
            text[..start].to_owned() + &replacement + &text[end..]
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn when_ast_cleanups_with_redundant_tokens_then_returns_the_text_gjf_cleans_it_to() {
        let cases = [
            ("class A {};\n;\nclass B {} ;\n", "class A {}\nclass B {}\n"),
            ("class A { int x;; }\n", "class A { int x;}\n"),
            ("enum E { X; ; }\n", "enum E { X;}\n"),
            ("enum F { ; }\n", "enum F {}\n"),
            ("enum G { ; void f() {} }\n", "enum G { ; void f() {} }\n"),
            (
                "enum E { X { ; void f() {}; }; }\n",
                "enum E { X { ; void f() {}}; }\n",
            ),
            (
                "enum E { X(() -> { f(); }) { ; void g() {} }; }\n",
                "enum E { X(() -> { f(); }) {void g() {} }; }\n",
            ),
            ("class A { void f() { ; } }\n", "class A { void f() {} }\n"),
            (
                "class A { void f() {\n  ;\n} }\n",
                "class A { void f() {\n} }\n",
            ),
            (
                "class A { void f() {\r\n  ;\r\n} }\n",
                "class A { void f() {\r\n} }\n",
            ),
            (
                "class A { void f() { for (;;) ; } }\n",
                "class A { void f() { for (;;) ; } }\n",
            ),
            ("@A() class A {}\n", "@A class A {}\n"),
            ("class A { @A()int x; }\n", "class A { @A int x; }\n"),
            ("@A(/* c */) class A {}\n", "@A(/* c */) class A {}\n"),
            ("@A(@B()) class A {}\n", "@A(@B) class A {}\n"),
            (
                "class A { @A\u{1d400}() int x; }\n",
                "class A { @A\u{1d400} int x; }\n",
            ),
            (
                "class A { @A\u{1d400}()int x; }\n",
                "class A { @A\u{1d400}int x; }\n",
            ),
            (
                "class A { Object o = f((x) -> x); }\n",
                "class A { Object o = f(x -> x); }\n",
            ),
            (
                "class A { Object o = (int x) -> x; }\n",
                "class A { Object o = (int x) -> x; }\n",
            ),
            (
                "class A { Object o = (/* c */x) -> x; }\n",
                "class A { Object o = (/* c */x) -> x; }\n",
            ),
        ];
        for (source, expected) in cases {
            let unit = parser::parse(source).ok();

            let cleaned = unit.map(|unit| ast_cleanups(source, &unit));

            assert_eq!(cleaned.as_deref(), Some(expected), "{source:?}");
        }
    }

    #[test]
    fn when_reorder_modifiers_with_modifier_runs_then_returns_them_in_jls_order() {
        let cases = [
            (
                "class A { static public final int X = 1; }\n",
                "class A { public static final int X = 1; }\n",
            ),
            (
                "class A { static /* c */ public int y; }\n",
                "class A { public /* c */ static int y; }\n",
            ),
            (
                "class A { static publi\\u0063 int x; }\n",
                "class A { public static int x; }\n",
            ),
            (
                "non-sealed abstract class C {}\n",
                "abstract non-sealed class C {}\n",
            ),
            (
                "abstract non-sealed class C {}\n",
                "abstract non-sealed class C {}\n",
            ),
            (
                "non-final abstract class C {}\n",
                "non-final abstract class C {}\n",
            ),
            (
                "class A { static @B public int x; }\n",
                "class A { static @B public int x; }\n",
            ),
            (
                "class A { void f() { switch (x) { default: } } }\n",
                "class A { void f() { switch (x) { default: } } }\n",
            ),
            ("non", "non"),
        ];
        for (source, expected) in cases {
            let reordered = reorder_modifiers(source);

            assert_eq!(reordered, Ok(expected.to_owned()), "{source:?}");
        }
    }
}
