use std::collections::HashSet;

use crate::lexer::token::{Keyword, Punct, Token, TokenClass, TokenKind};
use crate::parser;
use crate::parser::syntax::{Element, Node, NodeKind};
use crate::pipeline::{Position, Rejection};

pub fn apply(text: &str) -> Result<String, Rejection> {
    let tree = parser::parse(text)
        .map_err(|e| Rejection {
            position: Position::Offset(e.offset),
            message: e.message,
        })?
        .tree;
    let mut used: HashSet<String> = HashSet::new();
    let mut package = String::new();
    let mut imports: Vec<&Node> = Vec::new();
    let mut pending: Vec<&Node> = vec![&tree];
    while let Some(node) = pending.pop() {
        let first_identifier = node.children.iter().find_map(|child| match child {
            Element::Token(token) if token.kind == TokenKind::Identifier => {
                Some(&text[token.start..token.end])
            }
            _ => None,
        });
        if let NodeKind::CompilationUnit
        | NodeKind::PackageDecl
        | NodeKind::ModuleDecl
        | NodeKind::ClassDecl
        | NodeKind::InterfaceDecl
        | NodeKind::EnumDecl
        | NodeKind::RecordDecl
        | NodeKind::AnnotationTypeDecl
        | NodeKind::EnumConstant
        | NodeKind::FieldDecl
        | NodeKind::MethodDecl
        | NodeKind::ConstructorDecl
        | NodeKind::CompactConstructorDecl = node.kind
        {
            let mut doc: Option<String> = None;
            let mut markdown: Option<String> = None;
            for token in node
                .tokens()
                .into_iter()
                .take_while(|t| matches!(t.kind.class(), TokenClass::Trivia))
            {
                let content = &text[token.start..token.end];
                match (token.kind, content.strip_prefix("///")) {
                    (TokenKind::LineComment, Some(line)) => {
                        let group = markdown.get_or_insert_with(String::new);
                        group.push_str(line);
                        group.push('\n');
                    }
                    (TokenKind::Whitespace | TokenKind::Newline, _) => {}
                    (TokenKind::JavadocComment, _) => {
                        markdown = None;
                        doc = Some(content.to_owned());
                    }
                    _ => doc = markdown.take().or(doc),
                }
            }
            doc = markdown.map(|group| format!("///{group}")).or(doc);
            if let Some(comment) = doc {
                used.extend(javadoc_references(&comment));
            }
        }
        match (node.kind, first_identifier) {
            (NodeKind::ImportDecl, _) => {
                imports.push(node);
                continue;
            }
            (NodeKind::PackageDecl, _) => {
                package = node
                    .children
                    .iter()
                    .find_map(|child| match child {
                        Element::Node(inner) if inner.kind == NodeKind::QualifiedName => {
                            Some(inner)
                        }
                        _ => None,
                    })
                    .map(|name| {
                        name.tokens()
                            .iter()
                            .filter(|t| matches!(t.kind.class(), TokenClass::Significant))
                            .map(|t| &text[t.start..t.end])
                            .collect()
                    })
                    .unwrap_or_default();
            }
            (
                NodeKind::Name
                | NodeKind::ClassType
                | NodeKind::QualifiedName
                | NodeKind::ElementValuePair,
                Some(name),
            ) => {
                used.insert(name.to_owned());
            }
            _ => {}
        }
        for child in &node.children {
            if let Element::Node(inner) = child {
                pending.push(inner);
            }
        }
    }

    let mut removals: Vec<(usize, usize)> = Vec::new();
    for import in imports {
        let significant: Vec<Token> = import
            .tokens()
            .into_iter()
            .filter(|t| matches!(t.kind.class(), TokenClass::Significant))
            .collect();
        let names: Vec<&str> = significant
            .iter()
            .skip(1)
            .filter(|t| t.kind != TokenKind::Keyword(Keyword::Static))
            .take_while(|t| t.kind != TokenKind::Punct(Punct::Semicolon))
            .map(|t| &text[t.start..t.end])
            .collect();
        let module = matches!(
            (significant.get(1), significant.get(2)),
            (Some(word), Some(next)) if &text[word.start..word.end] == "module" && next.kind == TokenKind::Identifier
        );
        let Some((simple, qualifier_parts)) = names.split_last() else {
            continue;
        };
        let qualifier: String = qualifier_parts.concat();
        let qualifier = qualifier.trim_end_matches('.');
        let unused = match (module, qualifier, *simple) {
            (true, _, _) => false,
            (false, "java.lang", _) => true,
            (false, qualifier, _) if !package.is_empty() && qualifier == package => true,
            (false, _, "*") => false,
            (false, _, simple) => !used.contains(simple),
        };
        if let (true, Some(start), Some(end)) = (unused, significant.first(), significant.last()) {
            let mut stop = end.end;
            stop = text[stop..]
                .find(|c| c != ' ')
                .map_or(stop, |skip| stop + skip);
            if stop + 1 < text.len() && text.as_bytes()[stop] == b'\n' {
                stop += 1;
            }
            removals.push((start.start, stop));
        }
    }
    removals.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut position = 0;
    for (start, stop) in removals {
        out.push_str(&text[position..start]);
        position = stop;
    }
    out.push_str(&text[position..]);
    Ok(out)
}

fn identifier_length(text: &str) -> usize {
    match text
        .char_indices()
        .find(|&(_, c)| !(c.is_alphanumeric() || c == '_' || c == '$'))
    {
        Some((index, _)) => index,
        None => text.len(),
    }
}

fn reference(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth = 0usize;
    let end = content
        .char_indices()
        .find(|&(_, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            depth == 0 && c.is_whitespace()
        })
        .map_or(content.len(), |(i, _)| i);
    let target = &content[..end];
    let (qualifier, member) = target.split_once('#').unwrap_or((target, ""));
    let head = &qualifier[..identifier_length(qualifier)];
    if head
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
    {
        names.push(head.to_owned());
    }
    if let Some((_, parameters)) = member.split_once('(') {
        let parameters = parameters.trim_end_matches(')');
        let mut previous = ' ';
        let mut index = 0;
        while index < parameters.len() {
            let rest = &parameters[index..];
            let length = identifier_length(rest);
            match (length, previous) {
                (0, _) => {
                    previous = rest.chars().next().unwrap_or(' ');
                    index += previous.len_utf8();
                }
                (_, '.') => index += length,
                _ => {
                    let word = &rest[..length];
                    if !matches!(
                        word,
                        "boolean"
                            | "byte"
                            | "short"
                            | "int"
                            | "long"
                            | "char"
                            | "float"
                            | "double"
                            | "void"
                    ) && word.chars().next().is_some_and(|c| !c.is_ascii_digit())
                    {
                        names.push(word.to_owned());
                    }
                    previous = 'a';
                    index += length;
                }
            }
        }
    }
    names
}

fn doc_text(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0;
    let mut line_start = true;
    while let Some(c) = text[index..].chars().next() {
        let rest = &text[index..];
        match (c, line_start) {
            ('{', _) if rest[1..].starts_with('@') => {
                let mut depth = 0usize;
                let close = rest
                    .char_indices()
                    .find(|&(_, c)| {
                        match c {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map_or(rest.len(), |(i, _)| i);
                let inner = &rest[2..close];
                let name = &inner[..identifier_length(inner)];
                let content = inner[name.len()..].trim_start();
                match name {
                    "code" | "literal" => {}
                    "link" | "linkplain" => names.extend(reference(content)),
                    "value" => {
                        let content = match content.strip_prefix('"') {
                            Some(quoted) => quoted
                                .split_once('"')
                                .map_or("", |(_, after)| after.trim_start()),
                            None => content,
                        };
                        names.extend(reference(content));
                    }
                    _ => names.extend(doc_text(content)),
                }
                index += (close + 1).min(rest.len());
                line_start = false;
            }
            ('<', _) if rest.starts_with("<!--") => {
                index += rest.find("-->").map_or(rest.len(), |i| i + 3);
                line_start = false;
            }
            ('@', true) => {
                let name = &rest[1..1 + identifier_length(&rest[1..])];
                let after = &rest[1 + name.len()..];
                if let "see" | "throws" | "exception" = name {
                    names.extend(reference(after.trim_start()));
                }
                index += 1 + name.len();
                line_start = false;
            }
            ('\n', _) => {
                index += 1;
                line_start = true;
            }
            (c, _) => {
                index += c.len_utf8();
                line_start &= c.is_whitespace();
            }
        }
    }
    names
}

fn javadoc_references(raw: &str) -> Vec<String> {
    let markdown = raw.starts_with("///");
    let inner = match markdown {
        true => raw,
        false => raw.trim_start_matches("/**").trim_end_matches("*/"),
    };
    let body: String = inner
        .lines()
        .map(|line| match markdown {
            true => line.trim_start(),
            false => line.trim_start().trim_start_matches('*'),
        })
        .collect::<Vec<&str>>()
        .join("\n");
    let comment = body.as_str();
    let mut names = doc_text(comment);
    if markdown {
        let mut code = false;
        let mut previous = ' ';
        for (at, c) in comment.char_indices() {
            match (c, code, previous) {
                ('`', _, _) => code = !code,
                ('[', false, p) if p != '\\' => {
                    let inner_end = comment[at + 1..].find(']').map(|i| at + 1 + i);
                    if let Some(end) = inner_end {
                        let inner = &comment[at + 1..end];
                        let next = comment[end + 1..].chars().next();
                        let target = match next {
                            Some('[') => comment[end + 2..]
                                .find(']')
                                .map(|i| &comment[end + 2..end + 2 + i]),
                            Some('(') => None,
                            _ => Some(inner),
                        };
                        if let Some(target) = target {
                            names.extend(reference(target));
                        }
                    }
                }
                _ => {}
            }
            previous = c;
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_apply_with_imports_then_returns_the_source_without_unused_ones_as_gjf_does() {
        let cases = [
            (
                r#"import a.value;
@B(value = 1)
class A {}
"#,
                Some(
                    r#"import a.value;
@B(value = 1)
class A {}
"#,
                ),
            ),
            (
                r#"import a.Node;
class A { void Node() {} }
"#,
                Some(
                    r#"class A { void Node() {} }
"#,
                ),
            ),
            (
                r#"import a.FOO;
class A { void f(E e) { switch (e) { case FOO -> {} } } }
"#,
                Some(
                    r#"import a.FOO;
class A { void f(E e) { switch (e) { case FOO -> {} } } }
"#,
                ),
            ),
            (
                r#"import a.f;
class A { void g() { <T>f(); } }
"#,
                None,
            ),
            (
                r#"import a.B;
class A {
"#,
                None,
            ),
            (
                r#"/** {@link B} */
import a.B;
class A {}
"#,
                Some(
                    r#"/** {@link B} */
import a.B;
class A {}
"#,
                ),
            ),
            (
                r#"import a.B;
class A { void f() { /** {@link B} */ int x; } }
"#,
                Some(
                    r#"class A { void f() { /** {@link B} */ int x; } }
"#,
                ),
            ),
            (
                r#"import java.lang.String;
import java.lang.annotation.Retention;
class A { String s; Retention r; }
"#,
                Some(
                    r#"import java.lang.annotation.Retention;
class A { String s; Retention r; }
"#,
                ),
            ),
            (
                r#"import a.outer;
class A { void f() { outer: for (;;) break outer; } }
"#,
                Some(
                    r#"class A { void f() { outer: for (;;) break outer; } }
"#,
                ),
            ),
            (
                r#"import a.x;
class A { R r = x -> 1; }
"#,
                Some(
                    r#"class A { R r = x -> 1; }
"#,
                ),
            ),
            (
                r#"class A {}
import a.B;"#,
                None,
            ),
            (
                r#"import java.util.Map;
import java.util.Set;
/** {@link #f(Map, java.util.Set)} */
class A {}
"#,
                Some(
                    r#"import java.util.Map;
/** {@link #f(Map, java.util.Set)} */
class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;
/** {@link java.util.List} */
class A {}
"#,
                Some(
                    r#"/** {@link java.util.List} */
class A {}
"#,
                ),
            ),
            (
                r#"import a.B;
/** {@linkplain B label} */
class A {}
"#,
                Some(
                    r#"import a.B;
/** {@linkplain B label} */
class A {}
"#,
                ),
            ),
            (
                r#"import a.B;
class A { R r = B::new; }
"#,
                Some(
                    r#"import a.B;
class A { R r = B::new; }
"#,
                ),
            ),
            (
                r#"import module java.base;
class A {}
"#,
                Some(
                    r#"import module java.base;
class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;
class A { java.util.List l; }
"#,
                Some(
                    r#"class A { java.util.List l; }
"#,
                ),
            ),
            (
                r#"package a.b;
import a.b.C;
import a.b.c.D;
class A { C c; D d; }
"#,
                Some(
                    r#"package a.b;
import a.b.c.D;
class A { C c; D d; }
"#,
                ),
            ),
            (
                r#"import a.B;
/** @see "B" */
class A {}
"#,
                Some(
                    r#"/** @see "B" */
class A {}
"#,
                ),
            ),
            (
                r#"import static a.B.*;
import static a.C.d;
class A {}
"#,
                Some(
                    r#"import static a.B.*;
class A {}
"#,
                ),
            ),
            (
                r#"import a.E;
class A {
  /**
   * @throws E sometimes
   */
  void f() {}
}
"#,
                Some(
                    r#"import a.E;
class A {
  /**
   * @throws E sometimes
   */
  void f() {}
}
"#,
                ),
            ),
            (
                r#"import a.B;   
import a.C;
class A { C c; }
"#,
                Some(
                    r#"import a.C;
class A { C c; }
"#,
                ),
            ),
            (
                r#"package a;

import java.util.List;
import java.util.Map;

class A {}
"#,
                Some(
                    r#"package a;


class A {}
"#,
                ),
            ),
            (
                r#"import java.util.List;

/** See {@link List}. */
class A {}
"#,
                Some(
                    r#"import java.util.List;

/** See {@link List}. */
class A {}
"#,
                ),
            ),
            (
                r#"import java.util.Map;

/**
 * Docs.
 *
 * @see Map
 */
class A {}
"#,
                Some(
                    r#"import java.util.Map;

/**
 * Docs.
 *
 * @see Map
 */
class A {}
"#,
                ),
            ),
            (
                r#"import java.io.IOException;

class A {
  /** @throws IOException when it fails */
  void f() {}
}
"#,
                Some(
                    r#"import java.io.IOException;

class A {
  /** @throws IOException when it fails */
  void f() {}
}
"#,
                ),
            ),
            (
                r#"import java.util.List;

class A {
  String s = "List";
}
"#,
                Some(
                    r#"
class A {
  String s = "List";
}
"#,
                ),
            ),
            (
                r#"import a.b.Node;

class A {
  static class Node {}

  Node n;
}
"#,
                Some(
                    r#"import a.b.Node;

class A {
  static class Node {}

  Node n;
}
"#,
                ),
            ),
            (
                r#"import static java.lang.Math.max;
import static java.lang.Math.min;

class A {
  int x = max(1, 2);
}
"#,
                Some(
                    r#"import static java.lang.Math.max;

class A {
  int x = max(1, 2);
}
"#,
                ),
            ),
            (
                r#"import java.util.List;
import java.util.Map;

class A {
  List<String> l;
}
"#,
                Some(
                    r#"import java.util.List;

class A {
  List<String> l;
}
"#,
                ),
            ),
            (
                r#"import java.util.Map;

class A {
  Map.Entry<String, String> e;
}
"#,
                Some(
                    r#"import java.util.Map;

class A {
  Map.Entry<String, String> e;
}
"#,
                ),
            ),
            (
                r#"import javax.annotation.Nullable;
import java.util.List;

class A {
  @Nullable Object o;
}
"#,
                Some(
                    r#"import javax.annotation.Nullable;

class A {
  @Nullable Object o;
}
"#,
                ),
            ),
            (
                r#"import java.util.*;
import java.io.*;

class A {
  List<String> l;
}
"#,
                Some(
                    r#"import java.util.*;
import java.io.*;

class A {
  List<String> l;
}
"#,
                ),
            ),
            (
                r#"import a.Doc;
import a.Other;
/// See [Doc] and [text][Other] but not `[Code]` or \[1].
class A {}
"#,
                Some(
                    r#"import a.Doc;
import a.Other;
/// See [Doc] and [text][Other] but not `[Code]` or \[1].
class A {}
"#,
                ),
            ),
            (
                r#"import a.V;
/** {@value "%d" V#X} */
class A {}
"#,
                Some(
                    r#"import a.V;
/** {@value "%d" V#X} */
class A {}
"#,
                ),
            ),
            (
                "import a.B;\n/** {@code {@link B}} */ class A {}\n",
                Some("/** {@code {@link B}} */ class A {}\n"),
            ),
            (
                "import a.B;\n/** text @see B */ class A {}\n",
                Some("/** text @see B */ class A {}\n"),
            ),
            (
                "import a.B;\n/** <a href=\"x@see B\">x</a> */ class A {}\n",
                Some("/** <a href=\"x@see B\">x</a> */ class A {}\n"),
            ),
            (
                "import a.B;\n/**\n * <pre>{@code\n * {@link B}\n * }</pre>\n */\nclass A {}\n",
                Some("/**\n * <pre>{@code\n * {@link B}\n * }</pre>\n */\nclass A {}\n"),
            ),
            (
                "import a.B;\n/** {@summary uses {@link B}} */ class A {}\n",
                Some("import a.B;\n/** {@summary uses {@link B}} */ class A {}\n"),
            ),
            (
                "import a.B;\n/** if a < b then {@link B} */\nclass A {}\n",
                Some("import a.B;\n/** if a < b then {@link B} */\nclass A {}\n"),
            ),
            (
                "import a.B;\n/** <a title=\"{@link B}\">x</a> */\nclass A {}\n",
                Some("import a.B;\n/** <a title=\"{@link B}\">x</a> */\nclass A {}\n"),
            ),
            (
                "import a.B;\n/** <!-- {@link B} --> */\nclass A {}\n",
                Some("/** <!-- {@link B} --> */\nclass A {}\n"),
            ),
        ];
        for (input, expected) in cases {
            let output = apply(input).ok();

            assert_eq!(output.as_deref(), expected, "{input:?}");
        }
    }
}
