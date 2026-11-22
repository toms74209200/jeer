use super::builder::Unformatted;
use super::input;
use crate::lexer::identifier::{self, CharClass};
use crate::lexer::token::{Punct, Token, TokenClass, TokenKind};
use crate::lexer::{self};
use crate::parser::Unit;
use crate::syntax::{Element, Node, NodeKind};

struct Scanner<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    comments: Vec<usize>,
    deletions: Vec<(usize, usize)>,
}

impl Scanner<'_> {
    fn delete(&mut self, start: usize, end: usize) {
        self.deletions.push((start, end));
        self.deletions.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for &(start, end) in &self.deletions {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                Some(_) | None => merged.push((start, end)),
            }
        }
        self.deletions = merged;
    }

    fn delete_semicolon(&mut self, semicolon: usize) {
        let bytes = self.source.as_bytes();
        let horizontal = |b: u8| b == b' ' || b == b'\t';
        let mut start = semicolon;
        while start > 0 && horizontal(bytes[start - 1]) {
            start -= 1;
        }
        let mut line_begin = start;
        while line_begin > 0 && !matches!(bytes[line_begin - 1], b'\n' | b'\r') {
            line_begin -= 1;
        }
        let mut end = semicolon + 1;
        while end < bytes.len() && horizontal(bytes[end]) {
            end += 1;
        }
        let nothing_before = start == line_begin
            || self
                .deletions
                .iter()
                .any(|&(low, high)| low <= line_begin && start <= high);
        if nothing_before && end < bytes.len() {
            match bytes[end] {
                b'\r' => {
                    end += 1;
                    if bytes.get(end) == Some(&b'\n') {
                        end += 1;
                    }
                }
                b'\n' => end += 1,
                _ => {}
            }
        }
        self.delete(start, end);
    }

    fn semicolons_outside(&mut self, start: usize, end: usize, spans: &[(usize, usize)]) {
        let semicolons: Vec<usize> = self
            .tokens
            .iter()
            .filter(|token| {
                token.kind == TokenKind::Punct(Punct::Semicolon)
                    && (start..end).contains(&token.start)
                    && !spans
                        .iter()
                        .any(|&(low, high)| (low..high).contains(&token.start))
            })
            .map(|token| token.start)
            .collect();
        for semicolon in semicolons {
            self.delete_semicolon(semicolon);
        }
    }

    fn body_semicolons(&mut self, body: &Node) {
        let Some((start, end)) = span(body) else {
            return;
        };
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut last_constant_end = start;
        let mut first_member_start = end;
        let mut separator = None;
        for child in &body.children {
            match child {
                Element::Node(member) if member.kind != NodeKind::EmptyDecl => {
                    let Some((low, high)) = span(member) else {
                        continue;
                    };
                    spans.push((low, high));
                    match member.kind == NodeKind::EnumConstant {
                        true => last_constant_end = last_constant_end.max(high),
                        false => first_member_start = first_member_start.min(low),
                    }
                }
                Element::Node(_) => {}
                Element::Token(token) => {
                    if body.kind == NodeKind::EnumBody
                        && token.kind == TokenKind::Punct(Punct::Semicolon)
                        && separator.is_none()
                    {
                        separator = Some(token.start);
                    }
                }
            }
        }
        if let (NodeKind::EnumBody, false, Some(semicolon)) =
            (body.kind, spans.is_empty(), separator)
            && (last_constant_end..first_member_start).contains(&semicolon)
        {
            spans.push((semicolon, semicolon + 1));
        }
        self.semicolons_outside(start, end, &spans);
    }

    fn visit(&mut self, node: &Node) {
        if node.kind == NodeKind::CompilationUnit {
            let spans: Vec<(usize, usize)> = node
                .children
                .iter()
                .filter_map(|child| match child {
                    Element::Node(child) if child.kind != NodeKind::EmptyDecl => span(child),
                    Element::Node(_) | Element::Token(_) => None,
                })
                .collect();
            self.semicolons_outside(0, self.source.len(), &spans);
        } else if matches!(
            node.kind,
            NodeKind::ClassDecl
                | NodeKind::InterfaceDecl
                | NodeKind::EnumDecl
                | NodeKind::RecordDecl
                | NodeKind::AnnotationTypeDecl
        ) {
            let body = node.children.iter().find_map(|child| match child {
                Element::Node(body)
                    if matches!(body.kind, NodeKind::ClassBody | NodeKind::EnumBody) =>
                {
                    Some(body)
                }
                Element::Node(_) | Element::Token(_) => None,
            });
            if let Some(body) = body {
                self.body_semicolons(body);
            }
        }
        let anonymous = matches!(node.kind, NodeKind::NewClass | NodeKind::EnumConstant);
        for child in &node.children {
            let Element::Node(child) = child else {
                continue;
            };
            if anonymous && child.kind == NodeKind::ClassBody {
                self.body_semicolons(child);
            }
            if child.kind == NodeKind::EmptyStmt
                && matches!(node.kind, NodeKind::Block | NodeKind::SwitchGroup)
                && let Some((semicolon, _)) = span(child)
            {
                self.delete_semicolon(semicolon);
            }
            self.visit(child);
        }
        if node.kind == NodeKind::Annotation {
            let name_end = node.children.iter().find_map(|child| match child {
                Element::Node(name) if name.kind == NodeKind::QualifiedName => span(name),
                Element::Node(_) | Element::Token(_) => None,
            });
            let arguments = node.children.iter().find_map(|child| match child {
                Element::Node(arguments) if arguments.kind == NodeKind::AnnotationArguments => {
                    Some(arguments)
                }
                Element::Node(_) | Element::Token(_) => None,
            });
            if let (Some((_, name_end)), Some(arguments)) = (name_end, arguments)
                && arguments
                    .children
                    .iter()
                    .all(|child| matches!(child, Element::Token(_)))
                && let Some((_, end)) = span(arguments)
                && !self
                    .comments
                    .iter()
                    .any(|comment| (name_end..end).contains(comment))
            {
                self.delete(name_end, end);
            }
        } else if node.kind == NodeKind::Lambda {
            let significant: Vec<Token> = node
                .tokens()
                .into_iter()
                .filter(|token| matches!(token.kind.class(), TokenClass::Significant))
                .take(3)
                .collect();
            if let [open, _, close] = significant.as_slice()
                && open.kind == TokenKind::Punct(Punct::LParen)
                && close.kind == TokenKind::Punct(Punct::RParen)
                && !self
                    .comments
                    .iter()
                    .any(|comment| (open.start..close.end).contains(comment))
            {
                self.delete(open.start, open.end);
                self.delete(close.start, close.end);
            }
        }
    }
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

fn ast_cleanups(source: &str, unit: &Unit) -> String {
    let tokens = unit.tree.tokens();
    let comments = tokens
        .iter()
        .filter(|token| {
            matches!(
                token.kind,
                TokenKind::LineComment | TokenKind::BlockComment | TokenKind::JavadocComment
            )
        })
        .map(|token| token.start)
        .collect();
    let mut scanner = Scanner {
        source,
        tokens,
        comments,
        deletions: Vec::new(),
    };
    scanner.visit(&unit.tree);
    let mut cleaned = String::with_capacity(source.len());
    let mut cursor = 0;
    for &(start, end) in &scanner.deletions {
        cleaned.push_str(&source[cursor..start]);
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if let (Some(before), Some(after)) = (before, after)
            && identifier::class(before) != CharClass::Other
            && identifier::class(after) != CharClass::Other
        {
            cleaned.push(' ');
        }
        cursor = end;
    }
    cleaned.push_str(&source[cursor..]);
    cleaned
}

fn reorder_modifiers(source: &str) -> Result<String, Unformatted> {
    let lexed = lexer::scan(source).map_err(|error| Unformatted::Rejected {
        offset: error.offset,
        message: error.kind.to_string(),
    })?;
    let input = input::build(source, &lexed.tokens);
    let tokens = &input.tokens;
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
    let modifier_at = |index: usize| -> (Option<(usize, usize, usize)>, usize) {
        let text = tokens[index].tok.text.as_str();
        if text != "non" {
            let rank = order.iter().position(|name| *name == text);
            return (rank.map(|rank| (rank, index, index)), index + 1);
        }
        match (tokens.get(index + 1), tokens.get(index + 2)) {
            (Some(dash), Some(last)) if dash.tok.text == "-" => {
                let rank = match last.tok.text.as_str() {
                    "sealed" => order.iter().position(|name| *name == "non-sealed"),
                    _ => None,
                };
                (rank.map(|rank| (rank, index, index + 2)), index + 3)
            }
            (Some(_), _) => (None, index + 2),
            (None, _) => (None, index + 1),
        }
    };
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let (first, next) = modifier_at(index);
        index = next;
        let Some(first) = first else {
            continue;
        };
        let mut run = vec![first];
        while index < tokens.len() {
            let (modifier, next) = modifier_at(index);
            index = next;
            match modifier {
                Some(modifier) => run.push(modifier),
                None => break,
            }
        }
        if run.windows(2).all(|pair| pair[0].0 <= pair[1].0) {
            continue;
        }
        let mut sorted = run.clone();
        sorted.sort_by_key(|modifier| modifier.0);
        let mut replacement = String::new();
        for (position, modifier) in sorted.iter().enumerate() {
            let (_, first_token, last_token) = run[position];
            if position > 0 {
                for tok in &tokens[first_token].before {
                    replacement.push_str(&tok.text);
                }
            }
            replacement.push_str(order[modifier.0]);
            if position + 1 < sorted.len() {
                for tok in &tokens[last_token].after {
                    replacement.push_str(&tok.text);
                }
            }
        }
        let (_, begin, _) = run[0];
        let (_, _, end) = run[run.len() - 1];
        let end_tok = &tokens[end].tok;
        replacements.push((
            tokens[begin].tok.position,
            end_tok.position + end_tok.text.len(),
            replacement,
        ));
    }
    let mut reordered = source.to_owned();
    for (start, end, replacement) in replacements.into_iter().rev() {
        reordered.replace_range(start..end, &replacement);
    }
    Ok(reordered)
}

pub fn apply(source: &str, unit: &Unit) -> Result<String, Unformatted> {
    let cleaned = ast_cleanups(source, unit);
    reorder_modifiers(&cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn when_apply_with_each_cleanup_pass_then_returns_the_text_gjf_cleans_it_to() {
        let cases = [
            ("class A {};\n;\nclass B {} ;\n", "class A {}\nclass B {}\n"),
            ("class A { int x;; }\n", "class A { int x;}\n"),
            ("enum E { X; ; }\n", "enum E { X;}\n"),
            ("enum F { ; }\n", "enum F {}\n"),
            ("enum G { ; void f() {} }\n", "enum G { ; void f() {} }\n"),
            ("class A { void f() { ; } }\n", "class A { void f() {} }\n"),
            (
                "class A { void f() {\n  ;\n} }\n",
                "class A { void f() {\n} }\n",
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
            (
                "class A { static public final int X = 1; }\n",
                "class A { public static final int X = 1; }\n",
            ),
            (
                "class A { static /* c */ public int y; }\n",
                "class A { public /* c */ static int y; }\n",
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
                "class A { void f() { switch (x) { default: } } }\n",
                "class A { void f() { switch (x) { default: } } }\n",
            ),
        ];
        for (source, expected) in cases {
            let unit = parser::parse(source).ok();

            let cleaned = unit.map(|unit| apply(source, &unit));

            assert_eq!(cleaned, Some(Ok(expected.to_owned())), "{source:?}");
        }
    }
}
