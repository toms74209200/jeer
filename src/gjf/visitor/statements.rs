use super::super::builder::{Op, Unformatted};
use super::super::doc::HeaderMark;
use super::super::output::BlankLine;
use super::declarations::Declaration;
use super::{
    CLOSE, Direction, FORCED, OPEN_PLUS_TWO, OPEN_ZERO, PLUS_TWO, SPACE, SPACE_BREAK, Visitor, ZERO,
};
use crate::lexer::token::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Collapse {
    Empty,
    Never,
}

type Chain<'a> = (Vec<Clause<'a>>, Option<(&'a Token, &'a Node)>);

struct Clause<'a> {
    before: Option<&'a Token>,
    keyword: &'a Token,
    condition: &'a Node,
    then: &'a Node,
}
use crate::parser::syntax::{Element, Node, NodeKind};

impl<'a> Visitor<'a> {
    pub(super) fn block(
        &mut self,
        block: &'a Node,
        collapse: Collapse,
        leading: BlankLine,
        trailing: BlankLine,
    ) -> Result<(), Unformatted> {
        let [Element::Token(open), statements @ .., Element::Token(close)] =
            block.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        match (collapse, statements.is_empty()) {
            (Collapse::Empty, true) => {
                self.token(open, ZERO, Some(PLUS_TWO))?;
                self.builder.add(Op::BlankLine(BlankLine::No))?;
                self.token(close, PLUS_TWO, None)
            }
            (Collapse::Empty, false) | (Collapse::Never, _) => {
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(OPEN_PLUS_TWO)?;
                self.token(open, ZERO, Some(PLUS_TWO))?;
                self.builder.add(Op::BlankLine(leading))?;
                self.statements(statements)?;
                self.builder.add(CLOSE)?;
                self.builder.add(FORCED)?;
                self.builder.add(CLOSE)?;
                self.builder.add(Op::BlankLine(trailing))?;
                self.token(close, PLUS_TWO, None)
            }
        }
    }

    fn statement(
        &mut self,
        statement: &'a Node,
        collapse: Collapse,
        trailing: BlankLine,
    ) -> Result<(), Unformatted> {
        self.sync(statement)?;
        match statement.kind == NodeKind::Block {
            true => {
                self.builder.add(SPACE)?;
                self.block(statement, collapse, BlankLine::Preserve, trailing)
            }
            false => {
                self.builder.add(OPEN_PLUS_TWO)?;
                self.builder.add(SPACE_BREAK)?;
                self.scan(statement)?;
                self.builder.add(CLOSE)
            }
        }
    }

    fn header(&mut self, condition: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(open),
            Element::Node(inner),
            Element::Token(close),
        ] = condition.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(open, ZERO, None)?;
        self.builder.add(Op::Header(HeaderMark::Start))?;
        self.scan(inner)?;
        self.builder.add(Op::Header(HeaderMark::End))?;
        self.token(close, ZERO, None)
    }

    fn clauses(node: &'a Node, before: Option<&'a Token>) -> Result<Chain<'a>, Unformatted> {
        let (keyword, condition, then, otherwise) = match node.children.as_slice() {
            [
                Element::Token(keyword),
                Element::Node(condition),
                Element::Node(then),
            ] => (keyword, condition, then, None),
            [
                Element::Token(keyword),
                Element::Node(condition),
                Element::Node(then),
                Element::Token(word),
                Element::Node(otherwise),
            ] => (keyword, condition, then, Some((word, otherwise))),
            _ => return Err(Unformatted::Deferred),
        };
        let clause = Clause {
            before,
            keyword,
            condition,
            then,
        };
        match otherwise {
            Some((word, chained)) if chained.kind == NodeKind::IfStmt => {
                let (rest, last) = Self::clauses(chained, Some(word))?;
                Ok((std::iter::once(clause).chain(rest).collect(), last))
            }
            Some((word, last)) => Ok((vec![clause], Some((word, last)))),
            None => Ok((vec![clause], None)),
        }
    }

    pub(super) fn if_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let (clauses, last) = Self::clauses(node, None)?;
        let count = clauses.len();
        self.builder.add(OPEN_ZERO)?;
        for (index, clause) in clauses.iter().enumerate() {
            let following_block = index > 0 && clauses[index - 1].then.kind == NodeKind::Block;
            if let Some(word) = clause.before {
                match following_block {
                    true => self.builder.add(SPACE)?,
                    false => self.builder.add(FORCED)?,
                }
                self.token(word, ZERO, None)?;
                self.builder.add(SPACE)?;
            }
            self.token(clause.keyword, ZERO, None)?;
            self.builder.add(SPACE)?;
            self.header(clause.condition)?;
            let collapse = match count == 1 && last.is_none() {
                true => Collapse::Empty,
                false => Collapse::Never,
            };
            let trailing = match index + 1 < count || last.is_some() {
                true => BlankLine::Preserve,
                false => BlankLine::No,
            };
            self.statement(clause.then, collapse, trailing)?;
        }
        if let Some((word, otherwise)) = last {
            match clauses[count - 1].then.kind == NodeKind::Block {
                true => self.builder.add(SPACE)?,
                false => self.builder.add(FORCED)?,
            }
            self.token(word, ZERO, None)?;
            self.statement(otherwise, Collapse::Never, BlankLine::No)?;
        }
        self.builder.add(CLOSE)
    }

    pub(super) fn while_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Node(condition),
            Element::Node(body),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.header(condition)?;
        self.statement(body, Collapse::Empty, BlankLine::No)
    }

    pub(super) fn statements(&mut self, statements: &'a [Element]) -> Result<(), Unformatted> {
        for (index, statement) in statements.iter().enumerate() {
            let Element::Node(statement) = statement else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(FORCED)?;
            if index > 0 {
                self.builder.add(Op::BlankLine(BlankLine::Preserve))?;
            }
            if statement.kind == NodeKind::LocalVariableDecl {
                let Some(Element::Node(modifiers)) = statement.children.first() else {
                    return Err(Unformatted::Deferred);
                };
                let annotations: Vec<&Node> = modifiers
                    .children
                    .iter()
                    .filter_map(|child| match child {
                        Element::Node(annotation) => Some(annotation),
                        Element::Token(_) => None,
                    })
                    .collect();
                let parameterless = annotations
                    .iter()
                    .filter(|annotation| !annotation
                            .children.iter().any(|child| {
                            matches!(child, Element::Node(arguments) if arguments.kind == NodeKind::AnnotationArguments
                                && arguments.children.iter().any(|value| matches!(value, Element::Node(_))))
                        }))
                    .count();
                let direction = match parameterless <= 1 && parameterless == annotations.len() {
                    true => Direction::Horizontal,
                    false => Direction::Vertical,
                };
                self.variables(statement, Declaration::Local, direction)?;
            } else {
                self.scan(statement)?;
            }
        }
        Ok(())
    }

    pub(super) fn return_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        match node.children.as_slice() {
            [Element::Token(keyword), Element::Token(semicolon)] => {
                self.token(keyword, ZERO, None)?;
                self.token(semicolon, ZERO, None)
            }
            [
                Element::Token(keyword),
                Element::Node(expression),
                Element::Token(semicolon),
            ] => {
                self.token(keyword, ZERO, None)?;
                self.builder.add(SPACE)?;
                self.scan(expression)?;
                self.token(semicolon, ZERO, None)
            }
            _ => Err(Unformatted::Deferred),
        }
    }

    pub(super) fn expression_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [Element::Node(expression), Element::Token(semicolon)] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.scan(expression)?;
        self.token(semicolon, ZERO, None)
    }
}
