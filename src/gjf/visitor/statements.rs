use super::super::builder::{Op, Unformatted};
use super::super::doc::HeaderMark;
use super::super::output::BlankLine;
use super::declarations::{Declaration, field_annotation_direction};
use super::{
    BREAK, CLOSE, Direction, FILL, FILL_SPACE, FORCED, OPEN_PLUS_FOUR, OPEN_PLUS_TWO, OPEN_ZERO,
    PLUS_FOUR, PLUS_TWO, SPACE, SPACE_BREAK, Visitor, ZERO,
};
use crate::lexer::token::{Punct, Token, TokenKind};

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

    fn header(&mut self, parens: &'a [Element]) -> Result<(), Unformatted> {
        let [
            Element::Token(open),
            Element::Node(inner),
            Element::Token(close),
        ] = parens
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
            self.header(&clause.condition.children)?;
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
        self.header(&condition.children)?;
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
            Some(modifiers),
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

    pub(super) fn try_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [Element::Token(keyword), rest @ ..] = node.children.as_slice() else {
            return Err(Unformatted::Deferred);
        };
        let (resources, rest) = match rest {
            [Element::Node(spec), tail @ ..] if spec.kind == NodeKind::ResourceSpecification => {
                (Some(spec), tail)
            }
            tail => (None, tail),
        };
        let [Element::Node(body), clauses @ ..] = rest else {
            return Err(Unformatted::Deferred);
        };
        let clauses = clauses
            .iter()
            .map(|clause| match clause {
                Element::Node(clause) => Ok(clause),
                Element::Token(_) => Err(Unformatted::Deferred),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.builder.add(OPEN_ZERO)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        if let Some(resources) = resources {
            self.resources(resources)?;
        }
        let trailing = match clauses.is_empty() {
            true => (Collapse::Empty, BlankLine::No),
            false => (Collapse::Never, BlankLine::Preserve),
        };
        self.block(body, trailing.0, BlankLine::Preserve, trailing.1)?;
        for (index, clause) in clauses.iter().enumerate() {
            let trailing = match index + 1 < clauses.len() {
                true => BlankLine::Preserve,
                false => BlankLine::No,
            };
            match (
                clause.kind == NodeKind::CatchClause,
                clause.children.as_slice(),
            ) {
                (true, _) => self.catch_clause(clause, trailing)?,
                (false, [Element::Token(word), Element::Node(block)]) => {
                    self.builder.add(SPACE)?;
                    self.token(word, ZERO, None)?;
                    self.builder.add(SPACE)?;
                    self.block(block, Collapse::Never, BlankLine::Preserve, BlankLine::No)?;
                }
                (false, _) => return Err(Unformatted::Deferred),
            }
        }
        self.builder.add(CLOSE)
    }

    fn resources(&mut self, spec: &'a Node) -> Result<(), Unformatted> {
        let [Element::Token(open), items @ .., Element::Token(close)] = spec.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let count = items
            .iter()
            .filter(|item| matches!(item, Element::Node(_)))
            .count();
        self.token(open, ZERO, None)?;
        self.builder.add(Op::Header(HeaderMark::Start))?;
        match count > 1 {
            true => self.builder.add(OPEN_PLUS_FOUR)?,
            false => self.builder.add(OPEN_ZERO)?,
        }
        self.builder.add(Op::Fence)?;
        for (index, item) in items.iter().enumerate() {
            match item {
                Element::Token(semicolon) => {
                    self.token(semicolon, ZERO, None)?;
                    self.builder.add(SPACE)?;
                }
                Element::Node(resource) => {
                    if index > 0 {
                        self.builder.add(FORCED)?;
                    }
                    match resource.children.as_slice() {
                        [
                            Element::Node(modifiers),
                            Element::Node(ty),
                            Element::Token(name),
                            Element::Token(equals),
                            Element::Node(value),
                        ] => self.declare_one(
                            Declaration::Parameter,
                            field_annotation_direction(modifiers),
                            Some(modifiers),
                            ty,
                            None,
                            name,
                            &[],
                            Some((equals, value)),
                            None,
                        )?,
                        [Element::Node(expression)] => self.scan(expression)?,
                        _ => return Err(Unformatted::Deferred),
                    }
                }
            }
        }
        self.token(close, ZERO, None)?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Header(HeaderMark::End))?;
        self.builder.add(SPACE)
    }

    fn catch_clause(&mut self, clause: &'a Node, trailing: BlankLine) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Token(open),
            Element::Node(modifiers),
            Element::Node(types),
            Element::Token(name),
            Element::Token(close),
            Element::Node(block),
        ] = clause.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.builder.add(SPACE)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.token(open, ZERO, None)?;
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.builder.add(Op::Fence)?;
        match types.children.as_slice() {
            [Element::Node(ty)] => {
                self.builder.add(FILL)?;
                self.builder.add(OPEN_ZERO)?;
                self.declare_one(
                    Declaration::Local,
                    field_annotation_direction(modifiers),
                    Some(modifiers),
                    ty,
                    None,
                    name,
                    &[],
                    None,
                    None,
                )?;
                self.builder.add(CLOSE)?;
            }
            [alternatives @ .., Element::Token(bar), Element::Node(last)] => {
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(OPEN_ZERO)?;
                let annotations = self.modifiers(modifiers, Direction::Horizontal, None)?;
                self.inline_annotations(&annotations)?;
                if !annotations.is_empty() {
                    self.builder.add(FILL_SPACE)?;
                }
                for alternative in alternatives {
                    match alternative {
                        Element::Token(bar) => {
                            self.builder.add(SPACE_BREAK)?;
                            self.token(bar, ZERO, None)?;
                            self.builder.add(SPACE)?;
                        }
                        Element::Node(ty) => self.scan(ty)?,
                    }
                }
                self.builder.add(SPACE_BREAK)?;
                self.token(bar, ZERO, None)?;
                self.builder.add(SPACE)?;
                self.declare_one(
                    Declaration::Local,
                    Direction::Horizontal,
                    None,
                    last,
                    None,
                    name,
                    &[],
                    None,
                    None,
                )?;
                self.builder.add(CLOSE)?;
                self.builder.add(CLOSE)?;
            }
            _ => return Err(Unformatted::Deferred),
        }
        self.builder.add(CLOSE)?;
        self.token(close, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.block(block, Collapse::Never, BlankLine::Preserve, trailing)
    }

    pub(super) fn switch_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let (keyword, parens, block) = match node.children.as_slice() {
            [
                Element::Token(keyword),
                Element::Node(condition),
                Element::Node(block),
            ] => (keyword, condition.children.as_slice(), block),
            [Element::Token(keyword), parens @ .., Element::Node(block)] => {
                (keyword, parens, block)
            }
            _ => return Err(Unformatted::Deferred),
        };
        let [Element::Token(open), entries @ .., Element::Token(close)] = block.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.header(parens)?;
        self.builder.add(SPACE)?;
        self.token(open, ZERO, Some(PLUS_TWO))?;
        self.builder.add(Op::BlankLine(BlankLine::No))?;
        self.builder.add(OPEN_PLUS_TWO)?;
        for (index, entry) in entries.iter().enumerate() {
            let Element::Node(entry) = entry else {
                return Err(Unformatted::Deferred);
            };
            if index > 0 {
                self.builder.add(Op::BlankLine(BlankLine::Preserve))?;
            }
            self.case(entry)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(FORCED)?;
        self.builder.add(Op::BlankLine(BlankLine::No))?;
        self.token(close, PLUS_FOUR, None)
    }

    fn case(&mut self, entry: &'a Node) -> Result<(), Unformatted> {
        let [Element::Node(label), Element::Token(separator), rest @ ..] =
            entry.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let rule = entry.kind == NodeKind::SwitchRule;
        self.sync(entry)?;
        self.builder.add(FORCED)?;
        match rule {
            true => self.builder.add(OPEN_PLUS_FOUR)?,
            false => self.builder.add(OPEN_ZERO)?,
        }
        let (items, guard) = match label.children.as_slice() {
            [items @ .., Element::Node(guard)] if guard.kind == NodeKind::Guard => {
                (items, Some(guard))
            }
            items => (items, None),
        };
        match items {
            [Element::Token(word)] => self.token(word, ZERO, None)?,
            [Element::Token(word), labels @ ..] => {
                self.token(word, ZERO, None)?;
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(SPACE)?;
                for item in labels {
                    match item {
                        Element::Token(token) if token.kind == TokenKind::Punct(Punct::Comma) => {
                            self.token(token, ZERO, None)?;
                            self.builder.add(SPACE_BREAK)?;
                        }
                        Element::Token(token) => self.token(token, ZERO, None)?,
                        Element::Node(expression) => self.scan(expression)?,
                    }
                }
                self.builder.add(CLOSE)?;
            }
            _ => return Err(Unformatted::Deferred),
        }
        if let Some(guard) = guard {
            let [Element::Token(word), Element::Node(condition)] = guard.children.as_slice() else {
                return Err(Unformatted::Deferred);
            };
            self.builder.add(FILL_SPACE)?;
            self.token(word, ZERO, None)?;
            self.builder.add(SPACE)?;
            self.scan(condition)?;
        }
        match (rule, rest) {
            (false, statements) => {
                self.token(separator, ZERO, None)?;
                self.builder.add(OPEN_PLUS_TWO)?;
                self.statements(statements)?;
                self.builder.add(CLOSE)?;
                self.builder.add(CLOSE)
            }
            (true, [Element::Node(body)]) => {
                self.builder.add(SPACE)?;
                self.token(separator, ZERO, None)?;
                match (body.kind, body.children.as_slice()) {
                    (NodeKind::Block, _) => {
                        self.builder.add(CLOSE)?;
                        self.builder.add(SPACE)?;
                        self.block(body, Collapse::Empty, BlankLine::No, BlankLine::No)
                    }
                    (
                        NodeKind::ExpressionStmt,
                        [Element::Node(expression), Element::Token(semicolon)],
                    ) => {
                        self.builder.add(SPACE_BREAK)?;
                        self.scan(expression)?;
                        self.builder.add(CLOSE)?;
                        self.token(semicolon, ZERO, None)
                    }
                    (_, _) => {
                        self.builder.add(SPACE_BREAK)?;
                        self.scan(body)?;
                        self.builder.add(CLOSE)
                    }
                }
            }
            (true, _) => Err(Unformatted::Deferred),
        }
    }

    pub(super) fn assert_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let (keyword, condition, detail, semicolon) = match node.children.as_slice() {
            [
                Element::Token(keyword),
                Element::Node(condition),
                Element::Token(semicolon),
            ] => (keyword, condition, None, semicolon),
            [
                Element::Token(keyword),
                Element::Node(condition),
                Element::Token(colon),
                Element::Node(detail),
                Element::Token(semicolon),
            ] => (keyword, condition, Some((colon, detail)), semicolon),
            _ => return Err(Unformatted::Deferred),
        };
        self.builder.add(OPEN_ZERO)?;
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        match detail {
            Some(_) => self.builder.add(OPEN_PLUS_FOUR)?,
            None => self.builder.add(OPEN_ZERO)?,
        }
        self.builder.add(Op::Fence)?;
        self.scan(condition)?;
        if let Some((colon, detail)) = detail {
            self.builder.add(SPACE_BREAK)?;
            self.token(colon, ZERO, None)?;
            self.builder.add(SPACE)?;
            self.scan(detail)?;
        }
        self.builder.add(CLOSE)?;
        self.builder.add(CLOSE)?;
        self.token(semicolon, ZERO, None)
    }

    pub(super) fn throw_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Node(expression),
            Element::Token(semicolon),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(keyword, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.scan(expression)?;
        self.token(semicolon, ZERO, None)
    }

    pub(super) fn synchronized_statement(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Token(keyword),
            Element::Node(condition),
            Element::Node(block),
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
        self.builder.add(SPACE)?;
        self.token(open, ZERO, None)?;
        self.builder.add(Op::Header(HeaderMark::Start))?;
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.builder.add(BREAK)?;
        self.scan(inner)?;
        self.builder.add(CLOSE)?;
        self.builder.add(Op::Header(HeaderMark::End))?;
        self.token(close, ZERO, None)?;
        self.builder.add(SPACE)?;
        self.scan(block)
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
