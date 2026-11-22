use super::super::builder::{Op, Unformatted};
use super::super::doc::HeaderMark;
use super::super::output::BlankLine;
use super::declarations::Declaration;
use super::{
    CLOSE, Direction, FILL_SPACE, FORCED, OPEN_PLUS_FOUR, OPEN_PLUS_TWO, OPEN_ZERO, PLUS_TWO,
    SPACE, SPACE_BREAK, Visitor, ZERO,
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

    pub(super) fn for_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Token(open),
            Element::Node(init),
            Element::Token(first),
            rest @ ..,
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let (condition, second, update, close, body) = match rest {
            [
                Element::Node(condition),
                Element::Token(second),
                Element::Node(update),
                Element::Token(close),
                Element::Node(body),
            ] => (Some(condition), second, update, close, body),
            [
                Element::Token(second),
                Element::Node(update),
                Element::Token(close),
                Element::Node(body),
            ] => (None, second, update, close, body),
            _ => return Err(Unformatted::Deferred),
        };
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(open, ZERO, None)?;
        self.builder.add(Op::Header(HeaderMark::Start))?;
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.builder.add(Op::Fence)?;
        let expressions = init
            .children
            .iter()
            .filter(|child| matches!(child, Element::Node(_)))
            .count();
        let declared = matches!(
            init.children.as_slice(),
            [Element::Node(Node {
                kind: NodeKind::LocalVariableDecl,
                ..
            })]
        );
        match (declared, expressions > 1) {
            (false, true) => self.builder.add(OPEN_PLUS_FOUR)?,
            (false, false) | (true, _) => self.builder.add(OPEN_ZERO)?,
        }
        match init.children.as_slice() {
            [] => self.token(first, ZERO, None)?,
            [Element::Node(variable)] if declared => self.variable_list(
                variable,
                &variable.children,
                first,
                Declaration::Local,
                Direction::Horizontal,
            )?,
            items => {
                self.builder.add(OPEN_ZERO)?;
                self.list(items, SPACE_BREAK)?;
                self.token(first, ZERO, None)?;
                self.builder.add(CLOSE)?;
            }
        }
        self.builder.add(CLOSE)?;
        self.builder.add(SPACE_BREAK)?;
        if let Some(condition) = condition {
            self.scan(condition)?;
        }
        self.token(second, ZERO, None)?;
        let updates = update
            .children
            .iter()
            .filter(|child| matches!(child, Element::Node(_)))
            .count();
        match updates {
            0 => self.builder.add(SPACE)?,
            count => {
                self.builder.add(SPACE_BREAK)?;
                match count == 1 {
                    true => self.builder.add(OPEN_ZERO)?,
                    false => self.builder.add(OPEN_PLUS_FOUR)?,
                }
                self.list(&update.children, FILL_SPACE)?;
                self.builder.add(CLOSE)?;
            }
        }
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Header(HeaderMark::End))?;
        self.token(close, ZERO, None)?;
        self.statement(body, Collapse::Empty, BlankLine::No)
    }

    fn list(&mut self, items: &'a [Element], separator: Op<'static>) -> Result<(), Unformatted> {
        for item in items {
            match item {
                Element::Token(comma) => {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(separator.clone())?;
                }
                Element::Node(expression) => self.scan(expression)?,
            }
        }
        Ok(())
    }

    pub(super) fn for_each_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Token(open),
            Element::Node(variable),
            Element::Token(colon),
            Element::Node(iterable),
            Element::Token(close),
            Element::Node(body),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let [
            Element::Node(modifiers),
            Element::Node(ty),
            Element::Token(name),
            dims @ ..,
        ] = variable.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.builder.add(OPEN_ZERO)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(open, ZERO, None)?;
        self.builder.add(Op::Header(HeaderMark::Start))?;
        self.builder.add(OPEN_ZERO)?;
        self.sync(variable)?;
        self.declare_one(
            Declaration::Local,
            Direction::Horizontal,
            modifiers,
            ty,
            None,
            name,
            dims,
            Some((colon, iterable)),
            None,
        )?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Header(HeaderMark::End))?;
        self.token(close, ZERO, None)?;
        self.builder.add(CLOSE)?;
        self.statement(body, Collapse::Empty, BlankLine::No)
    }

    pub(super) fn do_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Node(body),
            Element::Token(word),
            Element::Node(condition),
            Element::Token(semicolon),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let [
            Element::Token(open),
            Element::Node(inner),
            Element::Token(close),
        ] = condition.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(keyword, ZERO, None)?;
        self.statement(body, Collapse::Empty, BlankLine::Preserve)?;
        match body.kind == NodeKind::Block {
            true => self.builder.add(SPACE)?,
            false => self.builder.add(SPACE_BREAK)?,
        }
        self.token(word, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(open, ZERO, None)?;
        self.scan(inner)?;
        self.token(close, ZERO, None)?;
        self.token(semicolon, ZERO, None)
    }

    pub(super) fn labeled_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(label),
            Element::Token(colon),
            Element::Node(statement),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.builder.add(OPEN_ZERO)?;
        self.token(label, ZERO, None)?;
        self.token(colon, ZERO, None)?;
        self.builder.add(FORCED)?;
        self.builder.add(CLOSE)?;
        self.scan(statement)
    }

    pub(super) fn jump_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let (keyword, label, semicolon) = match node.children.as_slice() {
            [Element::Token(keyword), Element::Token(semicolon)] => (keyword, None, semicolon),
            [
                Element::Token(keyword),
                Element::Token(label),
                Element::Token(semicolon),
            ] => (keyword, Some(label), semicolon),
            _ => return Err(Unformatted::Deferred),
        };
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.token(keyword, ZERO, None)?;
        if let Some(label) = label {
            self.builder.add(SPACE_BREAK)?;
            self.token(label, ZERO, None)?;
        }
        match node.kind == NodeKind::BreakStmt {
            true => {
                self.builder.add(CLOSE)?;
                self.token(semicolon, ZERO, None)
            }
            false => {
                self.token(semicolon, ZERO, None)?;
                self.builder.add(CLOSE)
            }
        }
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
