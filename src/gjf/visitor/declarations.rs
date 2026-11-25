use super::super::builder::{Op, Unformatted};
use super::super::doc::{BreakMode, Indent};
use super::super::input::{CommentKind, TokKind};
use super::super::output::BlankLine;
use super::statements::Collapse;
use super::{
    BREAK, CLOSE, Direction, FILL, FILL_SPACE, FORCED, MINUS_FOUR, MINUS_TWO, OPEN_PLUS_FOUR,
    OPEN_PLUS_TWO, OPEN_ZERO, PLUS_FOUR, PLUS_TWO, SPACE, SPACE_BREAK, Visitor, ZERO,
};
use crate::lexer::token::{Keyword, Punct, Token, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Declaration {
    Local,
    Field,
    Parameter,
}

const NODE: for<'n> fn(&'n Element) -> Option<&'n Node> = |item| match item {
    Element::Node(node) => Some(node),
    Element::Token(_) => None,
};

pub(super) fn field_annotation_direction(modifiers: &Node) -> Direction {
    let vertical = modifiers.children.iter().any(|modifier| {
        matches!(modifier, Element::Node(annotation) if annotation.children.iter().any(|child| {
                matches!(child, Element::Node(arguments) if arguments.kind == NodeKind::AnnotationArguments
                    && arguments.children.iter().any(|value| matches!(value, Element::Node(_))))
            }))
    });
    match vertical {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    }
}

fn optional(rest: &[Element], kind: NodeKind) -> (Option<&Node>, &[Element]) {
    match rest {
        [Element::Node(node), tail @ ..] if node.kind == kind => (Some(node), tail),
        rest => (None, rest),
    }
}

struct Declarator<'a> {
    name: &'a Token,
    dims: &'a [Element],
    initializer: Option<(&'a Token, &'a Node)>,
}

fn declarator(node: &Node) -> Result<Declarator<'_>, Unformatted> {
    let [Element::Token(name), rest @ ..] = node.children.as_slice() else {
        return Err(Unformatted::Deferred);
    };
    let (dims, initializer) = match rest {
        [dims @ .., Element::Token(equals), Element::Node(value)] => (dims, Some((equals, value))),
        dims => (dims, None),
    };
    Ok(Declarator {
        name,
        dims,
        initializer,
    })
}

impl<'a> Visitor<'a> {
    pub fn compilation_unit(&mut self, unit: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(unit)?;
        let implicit = unit.children.iter().any(|child| {
            matches!(
                child,
                Element::Node(Node {
                    kind: NodeKind::MethodDecl | NodeKind::FieldDecl | NodeKind::Initializer,
                    ..
                })
            )
        });
        let mut after_first = false;
        let mut members = Vec::new();
        for (index, child) in unit.children.iter().enumerate() {
            let Element::Node(node) = child else {
                return Err(Unformatted::Deferred);
            };
            if implicit && !matches!(node.kind, NodeKind::PackageDecl | NodeKind::ImportDecl) {
                members.push(node);
                continue;
            }
            if node.kind == NodeKind::PackageDecl {
                let [
                    Element::Node(modifiers),
                    Element::Token(keyword),
                    Element::Node(name),
                    Element::Token(semicolon),
                ] = node.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                let mut annotated = false;
                for annotation in &modifiers.children {
                    let Element::Node(annotation) = annotation else {
                        return Err(Unformatted::Deferred);
                    };
                    self.builder.add(FORCED)?;
                    self.annotation(annotation)?;
                    annotated = true;
                }
                if annotated {
                    self.builder.add(FORCED)?;
                }
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.token(keyword, ZERO, None)?;
                self.builder.add(SPACE)?;
                for part in &name.children {
                    let Element::Token(part) = part else {
                        return Err(Unformatted::Deferred);
                    };
                    self.token(part, ZERO, None)?;
                }
                self.builder.add(CLOSE)?;
                self.token(semicolon, ZERO, None)?;
            } else if node.kind == NodeKind::ImportDecl {
                let after_import = index > 0
                    && matches!(
                        &unit.children[index - 1],
                        Element::Node(Node {
                            kind: NodeKind::ImportDecl,
                            ..
                        })
                    );
                if after_first && !after_import {
                    self.builder.add(Op::BlankLine(BlankLine::Yes))?;
                }
                self.builder.add(Op::BlankLine(BlankLine::Preserve))?;
                self.sync(node)?;
                let tokens = node
                    .children
                    .iter()
                    .map(|child| match child {
                        Element::Token(token) => Ok(token),
                        Element::Node(_) => Err(Unformatted::Deferred),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let qualified: String = tokens[1..tokens.len() - 1]
                    .iter()
                    .filter(|token| token.kind != TokenKind::Keyword(Keyword::Static))
                    .map(|token| &self.source[token.start..token.end])
                    .collect();
                let type_annotations = [
                    "org.jspecify.annotations.NonNull",
                    "org.jspecify.annotations.Nullable",
                    "org.checkerframework.checker.nullness.qual.NonNull",
                    "org.checkerframework.checker.nullness.qual.Nullable",
                ];
                if type_annotations.contains(&qualified.as_str()) {
                    let simple = qualified.rsplit('.').next().unwrap_or_default();
                    self.type_annotations.insert(simple.to_owned());
                }
                let qualifier = match tokens.as_slice() {
                    [_, second, third, ..] => {
                        second.kind == TokenKind::Keyword(Keyword::Static)
                            || (&self.source[second.start..second.end] == "module"
                                && third.kind == TokenKind::Identifier)
                    }
                    _ => false,
                };
                for (position, token) in tokens.iter().enumerate() {
                    self.token(token, ZERO, None)?;
                    if position == 0 || (position == 1 && qualifier) {
                        self.builder.add(SPACE)?;
                    }
                }
            } else {
                if after_first {
                    self.builder.add(Op::BlankLine(BlankLine::Yes))?;
                }
                self.scan(node)?;
            }
            self.builder.add(FORCED)?;
            after_first = true;
        }
        if !members.is_empty() {
            if after_first {
                self.builder.add(Op::BlankLine(BlankLine::Yes))?;
            }
            self.builder.add(Op::Open(MINUS_TWO))?;
            self.members(&members, |_| Some(BlankLine::Preserve))?;
            self.builder.add(CLOSE)?;
            self.builder.add(FORCED)?;
        }
        Ok(())
    }

    fn markdown_javadoc(&mut self, node: &Node) -> Result<(), Unformatted> {
        let mut pending: Vec<&Element> = node.children.iter().rev().collect();
        let mut start = None;
        while let (None, Some(element)) = (start, pending.pop()) {
            match element {
                Element::Token(token) => start = Some(token.start),
                Element::Node(child) => pending.extend(child.children.iter().rev()),
            }
        }
        let tokens = &self.builder.input.tokens;
        let javadoc = start
            .and_then(|start| {
                let index = tokens.partition_point(|token| token.tok.position < start);
                tokens
                    .get(index)
                    .filter(|token| token.tok.position == start)
            })
            .and_then(|token| {
                token
                    .before
                    .iter()
                    .rev()
                    .find(|tok| tok.text.starts_with("/**") || tok.text.starts_with("///"))
            });
        match javadoc.map(|tok| (tok.kind, tok.position)) {
            Some((TokKind::Comment(CommentKind::MarkdownDoc, _), _)) => Err(Unformatted::Deferred),
            Some((TokKind::Comment(CommentKind::Line, _), position)) => {
                self.markdown_positions.insert(position);
                Ok(())
            }
            Some(_) | None => Ok(()),
        }
    }

    fn split_modifiers(&self, modifiers: &'a Node) -> (&'a [Element], Vec<&'a Node>) {
        let items = modifiers.children.as_slice();
        let suffix = items
            .iter()
            .rev()
            .take_while(|item| {
                if let Element::Node(annotation) = item
                    && annotation.kind == NodeKind::Annotation
                    && let Some(Element::Node(name)) = annotation.children.get(1)
                    && let [Element::Token(simple)] = name.children.as_slice()
                {
                    self.type_annotations
                        .contains(&self.source[simple.start..simple.end])
                } else {
                    false
                }
            })
            .count();
        let (declaration, annotations) = items.split_at(items.len() - suffix);
        let annotations = annotations.iter().filter_map(NODE).collect();
        (declaration, annotations)
    }

    pub(super) fn inline_annotations(
        &mut self,
        annotations: &[&'a Node],
    ) -> Result<(), Unformatted> {
        for (index, annotation) in annotations.iter().enumerate() {
            if index > 0 {
                self.builder.add(FILL_SPACE)?;
            }
            self.annotation(annotation)?;
        }
        Ok(())
    }

    fn vertical_annotations(&mut self, annotations: &[&'a Node]) -> Result<(), Unformatted> {
        for annotation in annotations {
            self.builder.add(FORCED)?;
            self.annotation(annotation)?;
            self.builder.add(FORCED)?;
        }
        Ok(())
    }

    pub(super) fn modifiers(
        &mut self,
        modifiers: &'a Node,
        direction: Direction,
        tag: Option<super::super::doc::BreakTag>,
    ) -> Result<Vec<&'a Node>, Unformatted> {
        let (items, annotations) = self.split_modifiers(modifiers);
        if items.is_empty() {
            return Ok(annotations);
        }
        let separator = match direction {
            Direction::Vertical => Op::Break {
                mode: BreakMode::Forced,
                flat: "",
                plus_indent: ZERO,
                tag,
            },
            Direction::Horizontal => Op::Break {
                mode: BreakMode::Unified,
                flat: " ",
                plus_indent: ZERO,
                tag,
            },
        };
        self.builder.add(OPEN_ZERO)?;
        let mut index = 0;
        while let Some(Element::Node(annotation)) = items.get(index) {
            if index > 0 {
                self.builder.add(separator.clone())?;
            }
            self.annotation(annotation)?;
            index += 1;
        }
        self.builder.add(CLOSE)?;
        if index == items.len() {
            self.builder.add(separator)?;
            return Ok(annotations);
        }
        if index > 0 {
            self.builder.add(separator)?;
        }
        self.builder.add(OPEN_ZERO)?;
        let mut first = true;
        while let Some(item) = items.get(index) {
            if !first {
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(FILL_SPACE)?;
                self.builder.add(CLOSE)?;
            }
            match item {
                Element::Token(token) if &self.source[token.start..token.end] == "non" => {
                    for part in items.get(index..index + 3).ok_or(Unformatted::Deferred)? {
                        let Element::Token(part) = part else {
                            return Err(Unformatted::Deferred);
                        };
                        self.token(part, ZERO, None)?;
                    }
                    index += 3;
                }
                Element::Token(token) => {
                    self.token(token, ZERO, None)?;
                    index += 1;
                }
                Element::Node(annotation) => {
                    self.annotation(annotation)?;
                    index += 1;
                }
            }
            first = false;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(OPEN_ZERO)?;
        self.builder.add(FILL_SPACE)?;
        self.builder.add(CLOSE)?;
        Ok(annotations)
    }

    pub(super) fn annotation(&mut self, annotation: &'a Node) -> Result<(), Unformatted> {
        self.sync(annotation)?;
        match annotation.children.as_slice() {
            [Element::Token(at), Element::Node(name)] => {
                self.builder.add(OPEN_ZERO)?;
                self.token(at, ZERO, None)?;
                self.qualified_name(name)?;
                self.builder.add(CLOSE)
            }
            [
                Element::Token(at),
                Element::Node(name),
                Element::Node(arguments),
            ] => {
                let values: Vec<&Node> = arguments
                    .children
                    .iter()
                    .filter_map(|child| match child {
                        Element::Node(node) => Some(node),
                        Element::Token(_) => None,
                    })
                    .collect();
                let (Some(Element::Token(open)), Some(Element::Token(close))) =
                    (arguments.children.first(), arguments.children.last())
                else {
                    return Err(Unformatted::Deferred);
                };
                match values.as_slice() {
                    [] => {
                        self.builder.add(OPEN_ZERO)?;
                        self.token(at, ZERO, None)?;
                        self.qualified_name(name)?;
                        self.token(open, ZERO, None)?;
                        self.token(close, ZERO, None)?;
                        self.builder.add(CLOSE)
                    }
                    [value] if value.kind != NodeKind::ElementValuePair => {
                        let array = value.kind == NodeKind::ElementValueArray;
                        match array {
                            true => self.builder.add(OPEN_ZERO)?,
                            false => self.builder.add(OPEN_PLUS_FOUR)?,
                        }
                        self.token(at, ZERO, None)?;
                        self.qualified_name(name)?;
                        self.token(open, ZERO, None)?;
                        if !array {
                            self.builder.add(BREAK)?;
                        }
                        self.element(value, Some(1))?;
                        self.builder.add(CLOSE)?;
                        self.token(close, ZERO, None)
                    }
                    _ => {
                        let has_array = values.iter().any(|value| {
                            value.kind == NodeKind::ElementValuePair
                                && matches!(
                                    value.children.last(),
                                    Some(Element::Node(Node {
                                        kind: NodeKind::ElementValueArray,
                                        ..
                                    }))
                                )
                        });
                        self.builder.add(OPEN_ZERO)?;
                        self.token(at, ZERO, None)?;
                        self.qualified_name(name)?;
                        self.builder.add(OPEN_PLUS_FOUR)?;
                        self.token(open, ZERO, None)?;
                        self.builder.add(BREAK)?;
                        for child in &arguments.children[1..arguments.children.len() - 1] {
                            match child {
                                Element::Token(comma) => {
                                    self.token(comma, ZERO, None)?;
                                    match has_array {
                                        true => self.builder.add(FORCED)?,
                                        false => self.builder.add(SPACE_BREAK)?,
                                    }
                                }
                                Element::Node(pair) if pair.kind == NodeKind::ElementValuePair => {
                                    self.sync(pair)?;
                                    let [
                                        Element::Token(key),
                                        Element::Token(equals),
                                        Element::Node(value),
                                    ] = pair.children.as_slice()
                                    else {
                                        return Err(Unformatted::Deferred);
                                    };
                                    let array = value.kind == NodeKind::ElementValueArray;
                                    match array {
                                        true => self.builder.add(OPEN_ZERO)?,
                                        false => self.builder.add(OPEN_PLUS_FOUR)?,
                                    }
                                    self.token(key, ZERO, None)?;
                                    self.builder.add(SPACE)?;
                                    self.token(equals, ZERO, None)?;
                                    match array {
                                        true => self.builder.add(SPACE)?,
                                        false => self.builder.add(SPACE_BREAK)?,
                                    }
                                    self.element(value, Some(1))?;
                                    self.builder.add(CLOSE)?;
                                }
                                Element::Node(value) => self.element(value, Some(1))?,
                            }
                        }
                        self.token(close, ZERO, None)?;
                        self.builder.add(CLOSE)?;
                        self.builder.add(CLOSE)
                    }
                }
            }
            _ => Err(Unformatted::Deferred),
        }
    }

    pub(super) fn class_declaration(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(node)?;
        let [
            Element::Node(modifiers),
            Element::Token(keyword),
            Element::Token(name),
            rest @ ..,
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let (type_parameters, rest) = optional(rest, NodeKind::TypeParameters);
        let (extends, rest) = optional(rest, NodeKind::ExtendsClause);
        let (implements, rest) = optional(rest, NodeKind::ImplementsClause);
        let (permits, rest) = optional(rest, NodeKind::PermitsClause);
        let [Element::Node(body)] = rest else {
            return Err(Unformatted::Deferred);
        };
        let (superclass, interfaces) = match keyword.kind == TokenKind::Keyword(Keyword::Interface)
        {
            true => (None, extends),
            false => (extends, implements),
        };
        let annotations = self.modifiers(modifiers, Direction::Vertical, None)?;
        self.vertical_annotations(&annotations)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(name, ZERO, None)?;
        if let Some(Element::Token(open)) = type_parameters.and_then(|tp| tp.children.first()) {
            self.token(open, ZERO, None)?;
        }
        self.builder.add(OPEN_PLUS_FOUR)?;
        if let Some(type_parameters) = type_parameters {
            let indent = match (superclass, interfaces, permits) {
                (None, None, None) => ZERO,
                _ => PLUS_FOUR,
            };
            self.type_parameters_rest(type_parameters, indent)?;
        }
        if let Some(clause) = superclass {
            let [Element::Token(keyword), Element::Node(ty)] = clause.children.as_slice() else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(FILL_SPACE)?;
            self.token(keyword, ZERO, None)?;
            self.builder.add(SPACE)?;
            self.scan(ty)?;
        }
        for clause in [interfaces, permits].into_iter().flatten() {
            self.type_clause(clause)?;
        }
        self.builder.add(CLOSE)?;
        self.class_body(body)
    }

    fn type_clause(&mut self, clause: &'a Node) -> Result<(), Unformatted> {
        let types = clause.children.iter().filter_map(NODE).count();
        self.builder.add(FILL_SPACE)?;
        match types > 1 {
            true => self.builder.add(OPEN_PLUS_FOUR)?,
            false => self.builder.add(OPEN_ZERO)?,
        }
        for (index, child) in clause.children.iter().enumerate() {
            match child {
                Element::Token(keyword) if index == 0 => {
                    self.token(keyword, ZERO, None)?;
                    self.builder.add(SPACE)?;
                }
                Element::Token(comma) => {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(SPACE_BREAK)?;
                }
                Element::Node(ty) => self.scan(ty)?,
            }
        }
        self.builder.add(CLOSE)
    }

    pub(super) fn record_declaration(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(node)?;
        let [
            Element::Node(modifiers),
            Element::Token(keyword),
            Element::Token(name),
            rest @ ..,
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let (type_parameters, rest) = optional(rest, NodeKind::TypeParameters);
        let [Element::Node(header), rest @ ..] = rest else {
            return Err(Unformatted::Deferred);
        };
        let (implements, rest) = optional(rest, NodeKind::ImplementsClause);
        let [Element::Node(body)] = rest else {
            return Err(Unformatted::Deferred);
        };
        let [Element::Token(open), components @ .., Element::Token(close)] =
            header.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let annotations = self.modifiers(modifiers, Direction::Vertical, None)?;
        self.vertical_annotations(&annotations)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(name, ZERO, None)?;
        if let Some(Element::Token(angle)) = type_parameters.and_then(|tp| tp.children.first()) {
            self.token(angle, ZERO, None)?;
        }
        self.builder.add(OPEN_PLUS_FOUR)?;
        if let Some(type_parameters) = type_parameters {
            let indent = match implements {
                Some(_) => PLUS_FOUR,
                None => ZERO,
            };
            self.type_parameters_rest(type_parameters, indent)?;
        }
        self.token(open, ZERO, None)?;
        if !components.is_empty() {
            self.builder.add(FILL)?;
            self.formals(components)?;
        }
        self.token(close, ZERO, None)?;
        if let Some(clause) = implements {
            self.type_clause(clause)?;
        }
        self.builder.add(CLOSE)?;
        self.class_body(body)
    }

    pub(super) fn class_body(&mut self, body: &'a Node) -> Result<(), Unformatted> {
        let [Element::Token(open), members @ .., Element::Token(close)] = body.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let mut nodes = Vec::new();
        for member in members {
            let Element::Node(member) = member else {
                return Err(Unformatted::Deferred);
            };
            nodes.push(member);
        }
        self.builder.add(SPACE)?;
        self.token(open, ZERO, Some(PLUS_TWO))?;
        match nodes.is_empty() {
            true => {
                self.builder.add(Op::BlankLine(BlankLine::No))?;
                self.builder.add(OPEN_ZERO)?;
            }
            false => {
                self.builder.add(OPEN_ZERO)?;
                self.members(&nodes, |_| Some(BlankLine::Preserve))?;
                self.builder.add(Op::BlankLine(BlankLine::No))?;
            }
        }
        self.token(close, PLUS_TWO, None)?;
        self.builder.add(CLOSE)
    }

    pub(super) fn type_parameters_rest(
        &mut self,
        type_parameters: &'a Node,
        plus_indent: Indent,
    ) -> Result<(), Unformatted> {
        self.builder.add(Op::Open(plus_indent))?;
        self.builder.add(BREAK)?;
        self.builder.add(OPEN_ZERO)?;
        for child in type_parameters.children.iter().skip(1) {
            match child {
                Element::Token(comma) if comma.kind == TokenKind::Punct(Punct::Comma) => {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(SPACE_BREAK)?;
                }
                Element::Token(close) => self.token(close, ZERO, None)?,
                Element::Node(parameter) => {
                    self.sync(parameter)?;
                    let [Element::Node(modifiers), Element::Token(name), bound @ ..] =
                        parameter.children.as_slice()
                    else {
                        return Err(Unformatted::Deferred);
                    };
                    self.builder.add(OPEN_ZERO)?;
                    for (index, annotation) in modifiers.children.iter().enumerate() {
                        let Element::Node(annotation) = annotation else {
                            return Err(Unformatted::Deferred);
                        };
                        if index > 0 {
                            self.builder.add(FILL_SPACE)?;
                        }
                        self.annotation(annotation)?;
                    }
                    if !modifiers.children.is_empty() {
                        self.builder.add(FILL_SPACE)?;
                    }
                    self.token(name, ZERO, None)?;
                    if let [Element::Node(bound)] = bound {
                        let [Element::Token(extends), types @ ..] = bound.children.as_slice()
                        else {
                            return Err(Unformatted::Deferred);
                        };
                        self.builder.add(SPACE)?;
                        self.token(extends, ZERO, None)?;
                        self.builder.add(OPEN_PLUS_FOUR)?;
                        self.builder.add(SPACE_BREAK)?;
                        self.builder.add(OPEN_PLUS_FOUR)?;
                        for child in types {
                            match child {
                                Element::Token(amp) => {
                                    self.builder.add(FILL_SPACE)?;
                                    self.token(amp, ZERO, None)?;
                                    self.builder.add(SPACE)?;
                                }
                                Element::Node(ty) => self.scan(ty)?,
                            }
                        }
                        self.builder.add(CLOSE)?;
                        self.builder.add(CLOSE)?;
                    }
                    self.builder.add(CLOSE)?;
                }
            }
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)
    }

    fn members(
        &mut self,
        members: &[&'a Node],
        first_blank_line: fn(bool) -> Option<BlankLine>,
    ) -> Result<(), Unformatted> {
        self.builder.add(OPEN_PLUS_TWO)?;
        let mut first = true;
        let mut last_got_blank = false;
        for member in members {
            self.builder.add(FORCED)?;
            if member.kind == NodeKind::EmptyDecl {
                for semicolon in &member.children {
                    let Element::Token(semicolon) = semicolon else {
                        return Err(Unformatted::Deferred);
                    };
                    self.token(semicolon, ZERO, None)?;
                }
                continue;
            }
            let javadoc = self
                .builder
                .input
                .tokens
                .get(self.builder.next)
                .is_some_and(|token| {
                    token
                        .before
                        .iter()
                        .any(|tok| tok.text.starts_with("/**") || tok.text.starts_with("///"))
                });
            let gets_blank = member.kind != NodeKind::FieldDecl || javadoc;
            let blank_line = match first {
                true => first_blank_line(gets_blank),
                false => (gets_blank || last_got_blank).then_some(BlankLine::Yes),
            };
            if let Some(blank_line) = blank_line {
                self.builder.add(Op::BlankLine(blank_line))?;
            }
            if member.kind == NodeKind::FieldDecl {
                let Some(Element::Node(modifiers)) = member.children.first() else {
                    return Err(Unformatted::Deferred);
                };
                self.variables(
                    member,
                    Declaration::Field,
                    field_annotation_direction(modifiers),
                )?;
            } else {
                self.scan(member)?;
            }
            first = false;
            last_got_blank = gets_blank;
        }
        self.builder.add(FORCED)?;
        self.builder.add(CLOSE)?;
        self.builder.add(FORCED)
    }

    pub(super) fn initializer(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [Element::Node(modifiers), Element::Node(block)] = node.children.as_slice() else {
            return Err(Unformatted::Deferred);
        };
        for modifier in &modifiers.children {
            let Element::Token(modifier) = modifier else {
                return Err(Unformatted::Deferred);
            };
            self.token(modifier, ZERO, None)?;
            self.builder.add(SPACE)?;
        }
        self.block(block, Collapse::Never, BlankLine::No, BlankLine::No)
    }

    pub(super) fn enum_declaration(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(node)?;
        let [
            Element::Node(modifiers),
            Element::Token(keyword),
            Element::Token(name),
            rest @ ..,
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let (implements, body) = match rest {
            [Element::Node(body)] => (None, body),
            [Element::Node(implements), Element::Node(body)]
                if implements.kind == NodeKind::ImplementsClause =>
            {
                (Some(implements), body)
            }
            _ => return Err(Unformatted::Deferred),
        };
        let [Element::Token(open), entries @ .., Element::Token(close)] = body.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let mut constants = Vec::new();
        let mut commas = Vec::new();
        let mut semicolon = None;
        let mut members = Vec::new();
        for entry in entries {
            match (entry, semicolon) {
                (Element::Node(constant), None) => constants.push(constant),
                (Element::Token(token), None) if token.kind == TokenKind::Punct(Punct::Comma) => {
                    commas.push(token);
                }
                (Element::Token(token), None) => semicolon = Some(token),
                (Element::Node(member), Some(_)) => members.push(member),
                (Element::Token(_), Some(_)) => return Err(Unformatted::Deferred),
            }
        }
        self.builder.add(OPEN_ZERO)?;
        let annotations = self.modifiers(modifiers, Direction::Vertical, None)?;
        self.vertical_annotations(&annotations)?;
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE_BREAK)?;
        self.token(name, ZERO, None)?;
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        if let Some(implements) = implements {
            let [Element::Token(keyword), types @ ..] = implements.children.as_slice() else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(OPEN_PLUS_FOUR)?;
            self.builder.add(SPACE_BREAK)?;
            self.builder.add(OPEN_PLUS_FOUR)?;
            self.token(keyword, ZERO, None)?;
            self.builder.add(SPACE_BREAK)?;
            self.builder.add(OPEN_ZERO)?;
            for child in types {
                match child {
                    Element::Token(comma) => {
                        self.token(comma, ZERO, None)?;
                        self.builder.add(FILL_SPACE)?;
                    }
                    Element::Node(ty) => self.scan(ty)?,
                }
            }
            self.builder.add(CLOSE)?;
            self.builder.add(CLOSE)?;
            self.builder.add(CLOSE)?;
        }
        self.builder.add(SPACE)?;
        self.token(open, ZERO, Some(PLUS_TWO))?;
        match (constants.is_empty() && members.is_empty(), semicolon) {
            (true, Some(_) | None) => {
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(Op::BlankLine(BlankLine::No))?;
                self.token(close, ZERO, None)?;
                self.builder.add(CLOSE)
            }
            (false, semicolon) => {
                self.builder.add(OPEN_PLUS_TWO)?;
                self.builder.add(Op::BlankLine(BlankLine::No))?;
                self.builder.add(FORCED)?;
                self.builder.add(OPEN_ZERO)?;
                for (index, constant) in constants.iter().enumerate() {
                    if let Some(comma) = index.checked_sub(1).and_then(|i| commas.get(i)) {
                        self.token(comma, ZERO, None)?;
                        self.builder.add(FORCED)?;
                        self.builder.add(Op::BlankLine(BlankLine::Preserve))?;
                    }
                    self.enum_constant(constant)?;
                }
                let trailing = match constants.len() {
                    0 => commas.first(),
                    count => commas.get(count - 1),
                };
                if let Some(comma) = trailing {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(FORCED)?;
                }
                self.builder.add(CLOSE)?;
                self.builder.add(CLOSE)?;
                if let Some(semicolon) = semicolon {
                    self.builder.add(OPEN_PLUS_TWO)?;
                    self.builder.add(Op::Fence)?;
                    self.token(semicolon, ZERO, None)?;
                    self.builder.add(FORCED)?;
                    self.builder.add(CLOSE)?;
                }
                self.builder.add(OPEN_ZERO)?;
                if !members.is_empty() {
                    self.members(&members, |gets_blank| gets_blank.then_some(BlankLine::Yes))?;
                }
                self.builder.add(FORCED)?;
                self.builder.add(Op::BlankLine(BlankLine::No))?;
                self.token(close, PLUS_TWO, None)?;
                self.builder.add(CLOSE)
            }
        }
    }

    fn enum_constant(&mut self, constant: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(constant)?;
        let [Element::Node(modifiers), Element::Token(name), rest @ ..] =
            constant.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        for annotation in &modifiers.children {
            let Element::Node(annotation) = annotation else {
                return Err(Unformatted::Deferred);
            };
            self.annotation(annotation)?;
            self.builder.add(FORCED)?;
        }
        self.token(name, ZERO, None)?;
        let bodied = rest.iter().any(|part| {
            matches!(
                part,
                Element::Node(Node {
                    kind: NodeKind::ClassBody,
                    ..
                })
            )
        });
        for part in rest {
            let Element::Node(part) = part else {
                return Err(Unformatted::Deferred);
            };
            if part.kind == NodeKind::ClassBody {
                self.class_body(part)?;
            } else if part.children.len() == 2 {
                for paren in &part.children {
                    let Element::Token(paren) = paren else {
                        return Err(Unformatted::Deferred);
                    };
                    self.token(paren, ZERO, None)?;
                }
            } else {
                if bodied {
                    self.do_not_hug(part);
                }
                self.arguments(part, PLUS_FOUR, ZERO)?;
            }
        }
        Ok(())
    }

    pub(super) fn annotation_type(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(node)?;
        let [
            Element::Node(modifiers),
            Element::Token(at),
            Element::Token(keyword),
            Element::Token(name),
            Element::Node(body),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.builder.add(OPEN_ZERO)?;
        let annotations = self.modifiers(modifiers, Direction::Vertical, None)?;
        self.vertical_annotations(&annotations)?;
        self.builder.add(OPEN_ZERO)?;
        self.token(at, ZERO, None)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE_BREAK)?;
        self.token(name, ZERO, None)?;
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        self.class_body(body)
    }

    pub(super) fn formals(&mut self, formals: &'a [Element]) -> Result<(), Unformatted> {
        self.builder.add(OPEN_ZERO)?;
        let mut first = true;
        let mut iter = formals.iter().peekable();
        while let Some(formal) = iter.next() {
            let Element::Node(formal) = formal else {
                return Err(Unformatted::Deferred);
            };
            let comma = match iter.peek() {
                Some(Element::Token(comma)) => {
                    iter.next();
                    Some(comma)
                }
                Some(Element::Node(_)) | None => None,
            };
            if !first {
                self.builder.add(SPACE_BREAK)?;
            }
            let [Element::Node(modifiers), Element::Node(ty), rest @ ..] =
                formal.children.as_slice()
            else {
                return Err(Unformatted::Deferred);
            };
            let (varargs, name, dims) = match rest {
                [Element::Token(varargs), Element::Token(name), dims @ ..]
                    if varargs.kind == TokenKind::Punct(Punct::Ellipsis) =>
                {
                    (Some(varargs), name, dims)
                }
                [Element::Token(name), dims @ ..] => (None, name, dims),
                _ => return Err(Unformatted::Deferred),
            };
            if !matches!(
                formal.kind,
                NodeKind::FormalParameter | NodeKind::RecordComponent
            ) {
                return Err(Unformatted::Deferred);
            }
            self.sync(formal)?;
            self.declare_one(
                Declaration::Parameter,
                Direction::Horizontal,
                Some(modifiers),
                ty,
                varargs,
                name,
                dims,
                None,
                comma,
            )?;
            first = false;
        }
        self.builder.add(CLOSE)
    }

    pub(super) fn method(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.markdown_javadoc(node)?;
        let [Element::Node(modifiers), rest @ ..] = node.children.as_slice() else {
            return Err(Unformatted::Deferred);
        };
        let (type_parameters, rest) = optional(rest, NodeKind::TypeParameters);
        let (return_type, rest) = match (node.kind == NodeKind::MethodDecl, rest) {
            (true, [Element::Node(return_type), tail @ ..]) => (Some(return_type), tail),
            (true, _) => return Err(Unformatted::Deferred),
            (false, rest) => (None, rest),
        };
        let (name, parameters, rest) = match (node.kind, rest) {
            (NodeKind::CompactConstructorDecl, [Element::Token(name), rest @ ..]) => {
                (name, None, rest)
            }
            (_, [Element::Token(name), Element::Node(parameters), rest @ ..]) => {
                (name, Some(parameters), rest)
            }
            _ => return Err(Unformatted::Deferred),
        };
        let dimensions = rest
            .iter()
            .take_while(|child| {
                matches!(
                    child,
                    Element::Node(Node {
                        kind: NodeKind::Dimension,
                        ..
                    })
                )
            })
            .count();
        let (dims, rest) = rest.split_at(dimensions);
        let (throws, rest) = optional(rest, NodeKind::ThrowsClause);
        let (body, default, semicolon) = match rest {
            [Element::Node(body)] if body.kind == NodeKind::Block => (Some(body), None, None),
            [Element::Node(default), Element::Token(semicolon)]
                if default.kind == NodeKind::DefaultValue =>
            {
                (None, Some(default), Some(semicolon))
            }
            [Element::Token(semicolon)] => (None, None, Some(semicolon)),
            _ => return Err(Unformatted::Deferred),
        };
        let parens = match parameters {
            Some(parameters) => {
                let [Element::Token(open), formals @ .., Element::Token(close)] =
                    parameters.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                Some((open, formals, close))
            }
            None => None,
        };
        let type_annotations = self.modifiers(modifiers, Direction::Vertical, None)?;
        let own = return_type.map_or(0, |return_type| {
            return_type
                .children
                .iter()
                .take_while(|child| {
                    matches!(
                        child,
                        Element::Node(Node {
                            kind: NodeKind::Annotation,
                            ..
                        })
                    )
                })
                .count()
        });
        let own_annotations: Vec<&'a Node> = return_type
            .into_iter()
            .flat_map(|return_type| &return_type.children[..own])
            .filter_map(NODE)
            .collect();
        let (leading, return_annotations) = match (type_parameters, return_type, own) {
            (_, None, _) => {
                self.vertical_annotations(&type_annotations)?;
                (Vec::new(), Vec::new())
            }
            (None, Some(_), _) => (Vec::new(), type_annotations),
            (Some(_), Some(_), 0) => (type_annotations, Vec::new()),
            (Some(_), Some(_), _) => (type_annotations, own_annotations),
        };
        self.builder.add(OPEN_PLUS_FOUR)?;
        let before_name = self.builder.tag();
        let before_type = self.builder.tag();
        self.builder.add(OPEN_ZERO)?;
        let mut after_first = !leading.is_empty();
        self.inline_annotations(&leading)?;
        if let Some(type_parameters) = type_parameters {
            let Some(Element::Token(angle)) = type_parameters.children.first() else {
                return Err(Unformatted::Deferred);
            };
            if after_first {
                self.builder.add(FILL_SPACE)?;
            }
            self.token(angle, ZERO, None)?;
            self.type_parameters_rest(type_parameters, PLUS_FOUR)?;
            after_first = true;
        }
        if let Some(return_type) = return_type {
            if after_first {
                self.builder.add(Op::Break {
                    mode: BreakMode::Fill,
                    flat: " ",
                    plus_indent: ZERO,
                    tag: Some(before_type),
                })?;
            }
            after_first = true;
            self.builder.add(Op::Open(Indent::If(
                before_type,
                Box::new(PLUS_FOUR),
                Box::new(ZERO),
            )))?;
            self.builder.add(OPEN_ZERO)?;
            if !return_annotations.is_empty() {
                self.inline_annotations(&return_annotations)?;
                self.builder.add(SPACE_BREAK)?;
            }
            match (own, return_type.kind) {
                (0, _) => self.typed(return_type)?,
                (_, kind) => {
                    let NodeKind::ClassType = kind else {
                        return Err(Unformatted::Deferred);
                    };
                    let segments = self.segments(&return_type.children[own..])?;
                    self.parameterized(&segments)?;
                }
            }
            self.builder.add(CLOSE)?;
        }
        if after_first {
            self.builder.add(Op::Break {
                mode: BreakMode::Fill,
                flat: " ",
                plus_indent: ZERO,
                tag: Some(before_name),
            })?;
        }
        if return_type.is_none() {
            self.builder.add(OPEN_ZERO)?;
        }
        self.token(name, ZERO, None)?;
        if let Some((open, _, _)) = parens {
            self.token(open, ZERO, None)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Open(Indent::If(
            before_name,
            Box::new(PLUS_FOUR),
            Box::new(ZERO),
        )))?;
        self.builder.add(Op::Open(Indent::If(
            before_type,
            Box::new(PLUS_FOUR),
            Box::new(ZERO),
        )))?;
        self.builder.add(OPEN_ZERO)?;
        if let Some((_, formals, close)) = parens {
            if !formals.is_empty() {
                self.builder.add(FILL)?;
                self.formals(formals)?;
            }
            self.token(close, ZERO, None)?;
        }
        self.dimensions(dims)?;
        if let Some(throws) = throws {
            let [Element::Token(keyword), types @ ..] = throws.children.as_slice() else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(FILL_SPACE)?;
            self.builder.add(OPEN_PLUS_FOUR)?;
            self.token(keyword, ZERO, None)?;
            self.builder.add(FILL_SPACE)?;
            for child in types {
                match child {
                    Element::Token(comma) => {
                        self.token(comma, ZERO, None)?;
                        self.builder.add(SPACE_BREAK)?;
                    }
                    Element::Node(ty) => self.scan(ty)?,
                }
            }
            self.builder.add(CLOSE)?;
        }
        if let Some(default) = default {
            let [Element::Token(keyword), Element::Node(value)] = default.children.as_slice()
            else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(SPACE)?;
            self.token(keyword, ZERO, None)?;
            match value.kind == NodeKind::ElementValueArray {
                true => {
                    self.builder.add(Op::Open(MINUS_FOUR))?;
                    self.builder.add(SPACE)?;
                    self.element(value, None)?;
                }
                false => {
                    self.builder.add(OPEN_ZERO)?;
                    self.builder.add(FILL_SPACE)?;
                    self.element(value, None)?;
                }
            }
            self.builder.add(CLOSE)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        match (body, semicolon) {
            (Some(body), _) => {
                let [Element::Token(open), statements @ .., Element::Token(close)] =
                    body.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                self.builder.add(SPACE)?;
                self.token(open, PLUS_TWO, Some(PLUS_TWO))?;
                self.builder.add(CLOSE)?;
                match statements.is_empty() {
                    true => self.builder.add(Op::BlankLine(BlankLine::No))?,
                    false => {
                        self.builder.add(OPEN_PLUS_TWO)?;
                        self.builder.add(FORCED)?;
                        self.builder.add(Op::BlankLine(BlankLine::Preserve))?;
                        self.statements(statements)?;
                        self.builder.add(CLOSE)?;
                        self.builder.add(FORCED)?;
                        self.builder.add(Op::BlankLine(BlankLine::No))?;
                    }
                }
                self.token(close, PLUS_TWO, None)
            }
            (None, Some(semicolon)) => {
                self.token(semicolon, ZERO, None)?;
                self.builder.add(CLOSE)
            }
            (None, None) => Err(Unformatted::Deferred),
        }
    }

    pub(super) fn variables(
        &mut self,
        node: &'a Node,
        declaration: Declaration,
        direction: Direction,
    ) -> Result<(), Unformatted> {
        let [declared @ .., Element::Token(semicolon)] = node.children.as_slice() else {
            return Err(Unformatted::Deferred);
        };
        if let (Declaration::Field, [_, _, Element::Node(_)]) = (&declaration, declared) {
            self.markdown_javadoc(node)?;
        }
        self.variable_list(node, declared, semicolon, declaration, direction)
    }

    pub(super) fn variable_list(
        &mut self,
        node: &'a Node,
        declared: &'a [Element],
        semicolon: &'a Token,
        declaration: Declaration,
        direction: Direction,
    ) -> Result<(), Unformatted> {
        self.sync(node)?;
        let [
            Element::Node(modifiers),
            Element::Node(ty),
            declarators @ ..,
        ] = declared
        else {
            return Err(Unformatted::Deferred);
        };
        match declarators {
            [Element::Node(single)] => {
                let Declarator {
                    name,
                    dims,
                    initializer,
                } = declarator(single)?;
                self.declare_one(
                    declaration,
                    direction,
                    Some(modifiers),
                    ty,
                    None,
                    name,
                    dims,
                    initializer,
                    Some(semicolon),
                )
            }
            _ => {
                self.builder.add(OPEN_ZERO)?;
                let annotations = self.modifiers(modifiers, direction, None)?;
                self.inline_annotations(&annotations)?;
                if !annotations.is_empty() {
                    self.builder.add(FILL_SPACE)?;
                }
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.builder.add(OPEN_ZERO)?;
                self.typed(ty)?;
                let mut first = true;
                for child in declarators {
                    match child {
                        Element::Token(comma) => self.token(comma, ZERO, None)?,
                        Element::Node(fragment) => {
                            let Declarator {
                                name,
                                dims,
                                initializer,
                            } = declarator(fragment)?;
                            self.builder.add(SPACE_BREAK)?;
                            self.builder.add(OPEN_ZERO)?;
                            self.token(name, ZERO, None)?;
                            self.dimensions(dims)?;
                            if let Some((equals, value)) = initializer {
                                self.builder.add(SPACE)?;
                                self.token(equals, ZERO, None)?;
                                self.builder.add(OPEN_PLUS_FOUR)?;
                                self.builder.add(SPACE_BREAK)?;
                                self.scan(value)?;
                                self.builder.add(CLOSE)?;
                            }
                            self.builder.add(CLOSE)?;
                            if first {
                                self.builder.add(CLOSE)?;
                            }
                            first = false;
                        }
                    }
                }
                self.builder.add(CLOSE)?;
                self.token(semicolon, ZERO, None)?;
                self.builder.add(CLOSE)
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments are those of google-java-format's declareOne"
    )]
    pub(super) fn declare_one(
        &mut self,
        declaration: Declaration,
        direction: Direction,
        modifiers: Option<&'a Node>,
        ty: &'a Node,
        varargs: Option<&'a Token>,
        name: &'a Token,
        dims: &'a [Element],
        initializer: Option<(&'a Token, &'a Node)>,
        trailing: Option<&'a Token>,
    ) -> Result<(), Unformatted> {
        let type_break = self.builder.tag();
        let annotation_break = self.builder.tag();
        let field_blank_line = match declaration {
            Declaration::Field => Some(BlankLine::Conditional(vec![annotation_break])),
            Declaration::Local | Declaration::Parameter => None,
        };
        if let Some(blank_line) = field_blank_line.clone() {
            self.builder.add(Op::BlankLine(blank_line))?;
        }
        let annotated = modifiers.is_some_and(|modifiers| {
            self.split_modifiers(modifiers)
                .0
                .iter()
                .filter_map(NODE)
                .count()
                > 0
        });
        match (declaration, annotated) {
            (Declaration::Parameter, true) => self.builder.add(OPEN_PLUS_FOUR)?,
            (Declaration::Parameter | Declaration::Field | Declaration::Local, _) => {
                self.builder.add(OPEN_ZERO)?;
            }
        }
        let annotations = match modifiers {
            Some(modifiers) => self.modifiers(modifiers, direction, Some(annotation_break))?,
            None => Vec::new(),
        };
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.builder.add(OPEN_ZERO)?;
        self.builder.add(OPEN_ZERO)?;
        self.inline_annotations(&annotations)?;
        if !annotations.is_empty() {
            self.builder.add(FILL_SPACE)?;
        }
        let (base, type_dims) = match ty.kind == NodeKind::ArrayType {
            true => match ty.children.as_slice() {
                [Element::Node(base), dims @ ..] => (base, dims),
                _ => return Err(Unformatted::Deferred),
            },
            false => (ty, &[] as &[Element]),
        };
        self.scan(base)?;
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.dimensions(type_dims)?;
        if let Some(varargs) = varargs {
            self.builder.add(FILL)?;
            self.token(varargs, ZERO, None)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Break {
            mode: BreakMode::Fill,
            flat: " ",
            plus_indent: ZERO,
            tag: Some(type_break),
        })?;
        self.builder.add(Op::Open(Indent::If(
            type_break,
            Box::new(PLUS_FOUR),
            Box::new(ZERO),
        )))?;
        self.token(name, ZERO, None)?;
        self.dimensions(dims)?;
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        if let Some((equals, value)) = initializer {
            self.builder.add(SPACE)?;
            self.token(equals, ZERO, None)?;
            match value.kind == NodeKind::ArrayInitializer {
                true => {
                    self.builder.add(Op::Open(MINUS_FOUR))?;
                    self.builder.add(SPACE)?;
                    self.array_initializer(value, None)?;
                }
                false => {
                    self.builder.add(Op::Open(Indent::If(
                        type_break,
                        Box::new(PLUS_FOUR),
                        Box::new(ZERO),
                    )))?;
                    self.builder.add(FILL_SPACE)?;
                    self.scan(value)?;
                }
            }
            self.builder.add(CLOSE)?;
        }
        if let Some(trailing) = trailing {
            self.token(trailing, ZERO, None)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        if let Some(blank_line) = field_blank_line {
            self.builder.add(Op::BlankLine(blank_line))?;
        }
        Ok(())
    }

    fn typed(&mut self, ty: &'a Node) -> Result<(), Unformatted> {
        match (ty.kind == NodeKind::ArrayType, ty.children.as_slice()) {
            (true, [Element::Node(base), dims @ ..]) => {
                self.scan(base)?;
                self.dimensions(dims)
            }
            (true, _) => Err(Unformatted::Deferred),
            (false, _) => self.scan(ty),
        }
    }

    pub(super) fn dimensions(&mut self, dims: &'a [Element]) -> Result<(), Unformatted> {
        for dim in dims {
            let Element::Node(dim) = dim else {
                return Err(Unformatted::Deferred);
            };
            let [Element::Token(open), Element::Token(close)] = dim.children.as_slice() else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(FILL)?;
            self.token(open, ZERO, None)?;
            self.token(close, ZERO, None)?;
        }
        Ok(())
    }
}
