use super::super::builder::{Op, Unformatted};
use super::super::output::BlankLine;
use super::declarations::Declaration;
use super::{CLOSE, Direction, FORCED, OPEN_PLUS_TWO, OPEN_ZERO, PLUS_TWO, SPACE, Visitor, ZERO};
use crate::syntax::{Element, Node, NodeKind};

impl<'a> Visitor<'a> {
    pub(super) fn block(&mut self, block: &'a Node) -> Result<(), Unformatted> {
        let [Element::Token(open), statements @ .., Element::Token(close)] =
            block.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.builder.add(OPEN_ZERO)?;
        self.builder.add(OPEN_PLUS_TWO)?;
        self.token(open, ZERO, Some(PLUS_TWO))?;
        self.builder.add(Op::BlankLine(BlankLine::No))?;
        self.statements(statements)?;
        self.builder.add(CLOSE)?;
        self.builder.add(FORCED)?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::BlankLine(BlankLine::No))?;
        self.token(close, PLUS_TWO, None)
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
                    .filter(|annotation| annotation.children.len() == 2)
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
