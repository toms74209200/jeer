use super::declarations::Modified;
use super::{ParseError, Parsed, Parser};
use crate::lexer::token::{Keyword, Punct, TokenClass, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};

enum Statement {
    LocalVariable,
    LocalClass,
    Other,
}

impl Parser<'_> {
    pub(super) fn block(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LBrace), &mut children)?;
        loop {
            match self.kind(0) {
                TokenKind::Punct(Punct::RBrace) => break,
                TokenKind::Eof => return self.fail("'}' expected"),
                _ => children.push(self.block_statement()?),
            }
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::Block,
            children,
        }))
    }

    fn statement_kind(&mut self) -> Statement {
        let declaration = self.probe(|p| {
            let mut ignored = Vec::new();
            let modified = p.modifiers(&mut ignored)?;
            match (p.kind(0), p.text(0), p.kind(1)) {
                (TokenKind::Keyword(Keyword::Class | Keyword::Interface | Keyword::Enum), _, _) => {
                    return Ok(Statement::LocalClass);
                }
                (TokenKind::Identifier, "record", TokenKind::Identifier) => {
                    return Ok(Statement::LocalClass);
                }
                (TokenKind::Punct(Punct::At), _, TokenKind::Keyword(Keyword::Interface)) => {
                    return Ok(Statement::LocalClass);
                }
                _ => {}
            }
            p.ty()?;
            match (p.kind(0), p.kind(1), modified) {
                (
                    TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore),
                    TokenKind::Punct(
                        Punct::Assign
                        | Punct::Comma
                        | Punct::Semicolon
                        | Punct::Colon
                        | Punct::LBracket,
                    ),
                    _,
                ) => Ok(Statement::LocalVariable),
                (
                    TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore),
                    _,
                    Modified::Yes,
                ) => Ok(Statement::LocalVariable),
                _ => Ok(Statement::Other),
            }
        });
        declaration.unwrap_or(Statement::Other)
    }

    pub(super) fn block_statement(&mut self) -> Parsed {
        match self.statement_kind() {
            Statement::LocalClass => {
                let mut children = Vec::new();
                self.modifiers(&mut children)?;
                let declaration = self.type_declaration(children)?;
                Ok(Element::Node(Node {
                    kind: NodeKind::LocalClassDecl,
                    children: vec![declaration],
                }))
            }
            Statement::LocalVariable => {
                let mut children = self.local_variable_head()?;
                self.variable_declarators(&mut children)?;
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                Ok(Element::Node(Node {
                    kind: NodeKind::LocalVariableDecl,
                    children,
                }))
            }
            Statement::Other => self.statement(),
        }
    }

    fn local_variable_head(&mut self) -> Result<Vec<Element>, ParseError> {
        let mut children = Vec::new();
        self.modifiers(&mut children)?;
        children.push(self.ty()?);
        Ok(children)
    }

    fn parenthesized(&mut self, out: &mut Vec<Element>) -> Result<(), ParseError> {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
        children.push(self.expression()?);
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        out.push(Element::Node(Node {
            kind: NodeKind::Parenthesized,
            children,
        }));
        Ok(())
    }

    fn statement_expression(expression: &Element) -> Result<(), ParseError> {
        let (kind, tokens) = match expression {
            Element::Node(node) => (Some(node.kind), node.tokens()),
            Element::Token(token) => (None, vec![*token]),
        };
        let mut significant = tokens
            .iter()
            .filter(|t| matches!(t.kind.class(), TokenClass::Significant));
        let first = significant.next();
        match (kind, first.map(|t| t.kind)) {
            (
                Some(
                    NodeKind::Assignment
                    | NodeKind::Postfix
                    | NodeKind::MethodCall
                    | NodeKind::NewClass,
                ),
                _,
            )
            | (
                Some(NodeKind::Unary),
                Some(TokenKind::Punct(Punct::PlusPlus | Punct::MinusMinus)),
            ) => Ok(()),
            _ => Err(ParseError {
                offset: first.map_or(0, |t| t.start),
                message: "not a statement".to_owned(),
            }),
        }
    }

    pub(super) fn statement(&mut self) -> Parsed {
        let mut children = Vec::new();
        let kind = match (self.kind(0), self.text(0), self.kind(1)) {
            (TokenKind::Punct(Punct::LBrace), _, _) => return self.block(),
            (TokenKind::Punct(Punct::Semicolon), _, _) => {
                self.bump(&mut children);
                NodeKind::EmptyStmt
            }
            (TokenKind::Keyword(Keyword::If), _, _) => {
                self.bump(&mut children);
                self.parenthesized(&mut children)?;
                children.push(self.statement()?);
                if self.kind(0) == TokenKind::Keyword(Keyword::Else) {
                    self.bump(&mut children);
                    children.push(self.statement()?);
                }
                NodeKind::IfStmt
            }
            (TokenKind::Keyword(Keyword::While), _, _) => {
                self.bump(&mut children);
                self.parenthesized(&mut children)?;
                children.push(self.statement()?);
                NodeKind::WhileStmt
            }
            (TokenKind::Keyword(Keyword::Do), _, _) => {
                self.bump(&mut children);
                children.push(self.statement()?);
                self.expect(TokenKind::Keyword(Keyword::While), &mut children)?;
                self.parenthesized(&mut children)?;
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::DoStmt
            }
            (TokenKind::Keyword(Keyword::For), _, _) => return self.for_statement(),
            (TokenKind::Keyword(Keyword::Try), _, _) => return self.try_statement(),
            (TokenKind::Keyword(Keyword::Switch), _, _) => {
                self.bump(&mut children);
                self.parenthesized(&mut children)?;
                let block = self.switch_block()?;
                if let Element::Node(switch_block) = &block {
                    for entry in &switch_block.children {
                        if let Element::Node(Node {
                            kind: NodeKind::SwitchRule,
                            children: rule,
                        }) = entry
                        {
                            for body in rule {
                                if let Element::Node(Node {
                                    kind: NodeKind::ExpressionStmt,
                                    children: expression,
                                }) = body
                                {
                                    Self::statement_expression(&expression[0])?;
                                }
                            }
                        }
                    }
                }
                children.push(block);
                NodeKind::SwitchStmt
            }
            (TokenKind::Keyword(Keyword::Return), _, _) => {
                self.bump(&mut children);
                if self.kind(0) != TokenKind::Punct(Punct::Semicolon) {
                    children.push(self.expression()?);
                }
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::ReturnStmt
            }
            (TokenKind::Keyword(Keyword::Break | Keyword::Continue), _, _) => {
                let kind = match self.kind(0) {
                    TokenKind::Keyword(Keyword::Break) => NodeKind::BreakStmt,
                    _ => NodeKind::ContinueStmt,
                };
                self.bump(&mut children);
                if self.kind(0) == TokenKind::Identifier {
                    self.bump(&mut children);
                }
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                kind
            }
            (TokenKind::Keyword(Keyword::Throw), _, _) => {
                self.bump(&mut children);
                children.push(self.expression()?);
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::ThrowStmt
            }
            (TokenKind::Keyword(Keyword::Synchronized), _, _) => {
                self.bump(&mut children);
                self.parenthesized(&mut children)?;
                children.push(self.block()?);
                NodeKind::SynchronizedStmt
            }
            (TokenKind::Keyword(Keyword::Assert), _, _) => {
                self.bump(&mut children);
                children.push(self.expression()?);
                if self.kind(0) == TokenKind::Punct(Punct::Colon) {
                    self.bump(&mut children);
                    children.push(self.expression()?);
                }
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::AssertStmt
            }
            (TokenKind::Keyword(Keyword::Case | Keyword::Default), _, _) => {
                return self.fail("orphaned case");
            }
            (TokenKind::Keyword(Keyword::Else), _, _) => return self.fail("'else' without 'if'"),
            (TokenKind::Identifier, _, TokenKind::Punct(Punct::Colon)) => {
                self.bump(&mut children);
                self.bump(&mut children);
                children.push(self.statement()?);
                NodeKind::LabeledStmt
            }
            (TokenKind::Identifier, "yield", next)
                if !matches!(
                    next,
                    TokenKind::Punct(
                        Punct::Assign
                            | Punct::PlusEq
                            | Punct::MinusEq
                            | Punct::StarEq
                            | Punct::SlashEq
                            | Punct::PercentEq
                            | Punct::AmpEq
                            | Punct::PipeEq
                            | Punct::CaretEq
                            | Punct::LtLtEq
                            | Punct::GtGtEq
                            | Punct::GtGtGtEq
                            | Punct::Dot
                            | Punct::LBracket
                            | Punct::PlusPlus
                            | Punct::MinusMinus
                            | Punct::ColonColon
                            | Punct::Semicolon
                            | Punct::Arrow
                    )
                ) =>
            {
                self.bump(&mut children);
                children.push(self.expression()?);
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::YieldStmt
            }
            _ => {
                let expression = self.expression()?;
                Self::statement_expression(&expression)?;
                children.push(expression);
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                NodeKind::ExpressionStmt
            }
        };
        Ok(Element::Node(Node { kind, children }))
    }

    fn for_statement(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
        let each = self.probe(|p| {
            let mut ignored = p.local_variable_head()?;
            p.expect_identifier(&mut ignored)?;
            p.dimensions(&mut ignored)?;
            Ok(p.kind(0))
        });
        if let Ok(TokenKind::Punct(Punct::Colon)) = each {
            let mut variable = self.local_variable_head()?;
            self.expect_identifier(&mut variable)?;
            self.dimensions(&mut variable)?;
            children.push(Element::Node(Node {
                kind: NodeKind::LocalVariableDecl,
                children: variable,
            }));
            self.bump(&mut children);
            children.push(self.expression()?);
            self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
            children.push(self.statement()?);
            return Ok(Element::Node(Node {
                kind: NodeKind::ForEachStmt,
                children,
            }));
        }
        let mut init = Vec::new();
        match (self.kind(0), self.statement_kind()) {
            (TokenKind::Punct(Punct::Semicolon), _) => {}
            (_, Statement::LocalVariable) => {
                let mut variable = self.local_variable_head()?;
                self.variable_declarators(&mut variable)?;
                init.push(Element::Node(Node {
                    kind: NodeKind::LocalVariableDecl,
                    children: variable,
                }));
            }
            _ => self.expression_list(&mut init)?,
        }
        children.push(Element::Node(Node {
            kind: NodeKind::ForInit,
            children: init,
        }));
        self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
        if self.kind(0) != TokenKind::Punct(Punct::Semicolon) {
            children.push(self.expression()?);
        }
        self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
        let mut update = Vec::new();
        if self.kind(0) != TokenKind::Punct(Punct::RParen) {
            self.expression_list(&mut update)?;
        }
        children.push(Element::Node(Node {
            kind: NodeKind::ForUpdate,
            children: update,
        }));
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        children.push(self.statement()?);
        Ok(Element::Node(Node {
            kind: NodeKind::ForStmt,
            children,
        }))
    }

    fn expression_list(&mut self, out: &mut Vec<Element>) -> Result<(), ParseError> {
        loop {
            let expression = self.expression()?;
            Self::statement_expression(&expression)?;
            out.push(expression);
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(out),
                _ => return Ok(()),
            }
        }
    }

    fn try_statement(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        if self.kind(0) == TokenKind::Punct(Punct::LParen) {
            let mut resources = Vec::new();
            self.bump(&mut resources);
            while self.kind(0) != TokenKind::Punct(Punct::RParen) {
                let resource = match self.statement_kind() {
                    Statement::LocalVariable => {
                        let mut resource = self.local_variable_head()?;
                        self.expect_identifier(&mut resource)?;
                        self.expect(TokenKind::Punct(Punct::Assign), &mut resource)?;
                        resource.push(self.expression()?);
                        resource
                    }
                    Statement::LocalClass | Statement::Other => vec![self.expression()?],
                };
                resources.push(Element::Node(Node {
                    kind: NodeKind::Resource,
                    children: resource,
                }));
                match self.kind(0) {
                    TokenKind::Punct(Punct::Semicolon) => self.bump(&mut resources),
                    _ => break,
                }
            }
            self.expect(TokenKind::Punct(Punct::RParen), &mut resources)?;
            children.push(Element::Node(Node {
                kind: NodeKind::ResourceSpecification,
                children: resources,
            }));
        }
        children.push(self.block()?);
        while self.kind(0) == TokenKind::Keyword(Keyword::Catch) {
            let mut catch = Vec::new();
            self.bump(&mut catch);
            self.expect(TokenKind::Punct(Punct::LParen), &mut catch)?;
            self.modifiers(&mut catch)?;
            let mut types = vec![self.ty()?];
            while self.kind(0) == TokenKind::Punct(Punct::Pipe) {
                self.bump(&mut types);
                types.push(self.ty()?);
            }
            catch.push(Element::Node(Node {
                kind: NodeKind::CatchType,
                children: types,
            }));
            self.expect_identifier(&mut catch)?;
            self.expect(TokenKind::Punct(Punct::RParen), &mut catch)?;
            catch.push(self.block()?);
            children.push(Element::Node(Node {
                kind: NodeKind::CatchClause,
                children: catch,
            }));
        }
        if self.kind(0) == TokenKind::Keyword(Keyword::Finally) {
            let mut finally = Vec::new();
            self.bump(&mut finally);
            finally.push(self.block()?);
            children.push(Element::Node(Node {
                kind: NodeKind::FinallyClause,
                children: finally,
            }));
        }
        Ok(Element::Node(Node {
            kind: NodeKind::TryStmt,
            children,
        }))
    }

    pub(super) fn switch_block(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LBrace), &mut children)?;
        while self.kind(0) != TokenKind::Punct(Punct::RBrace) {
            let mut entry = vec![self.switch_label()?];
            match self.kind(0) {
                TokenKind::Punct(Punct::Arrow) => {
                    self.bump(&mut entry);
                    match self.kind(0) {
                        TokenKind::Punct(Punct::LBrace) => entry.push(self.block()?),
                        TokenKind::Keyword(Keyword::Throw) => entry.push(self.statement()?),
                        _ => {
                            let mut expression = vec![self.expression()?];
                            self.expect(TokenKind::Punct(Punct::Semicolon), &mut expression)?;
                            entry.push(Element::Node(Node {
                                kind: NodeKind::ExpressionStmt,
                                children: expression,
                            }));
                        }
                    }
                    children.push(Element::Node(Node {
                        kind: NodeKind::SwitchRule,
                        children: entry,
                    }));
                }
                TokenKind::Punct(Punct::Colon) => {
                    self.bump(&mut entry);
                    loop {
                        match self.kind(0) {
                            TokenKind::Keyword(Keyword::Case) | TokenKind::Punct(Punct::RBrace) => {
                                break;
                            }
                            TokenKind::Keyword(Keyword::Default)
                                if matches!(
                                    self.kind(1),
                                    TokenKind::Punct(Punct::Colon | Punct::Arrow)
                                ) =>
                            {
                                break;
                            }
                            TokenKind::Eof => return self.fail("'}' expected"),
                            _ => entry.push(self.block_statement()?),
                        }
                    }
                    children.push(Element::Node(Node {
                        kind: NodeKind::SwitchGroup,
                        children: entry,
                    }));
                }
                _ => return self.fail("':' or '->' expected"),
            }
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::SwitchBlock,
            children,
        }))
    }

    fn switch_label(&mut self) -> Parsed {
        let mut children = Vec::new();
        match self.kind(0) {
            TokenKind::Keyword(Keyword::Default) => self.bump(&mut children),
            TokenKind::Keyword(Keyword::Case) => {
                self.bump(&mut children);
                loop {
                    match self.kind(0) {
                        TokenKind::Keyword(Keyword::Default) => self.bump(&mut children),
                        _ => children.push(self.case_element()?),
                    }
                    match self.kind(0) {
                        TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                        _ => break,
                    }
                }
                if self.text(0) == "when" && self.kind(0) == TokenKind::Identifier {
                    let mut guard = Vec::new();
                    self.bump(&mut guard);
                    guard.push(self.expression()?);
                    children.push(Element::Node(Node {
                        kind: NodeKind::Guard,
                        children: guard,
                    }));
                }
            }
            _ => return self.fail("case, default, or '}' expected"),
        }
        Ok(Element::Node(Node {
            kind: NodeKind::SwitchLabel,
            children,
        }))
    }

    fn case_element(&mut self) -> Parsed {
        let pattern = self.probe(|p| {
            let mut ignored = Vec::new();
            p.modifiers(&mut ignored)?;
            p.ty()?;
            Ok(p.kind(0))
        });
        match pattern {
            Ok(
                TokenKind::Identifier
                | TokenKind::Keyword(Keyword::Underscore)
                | TokenKind::Punct(Punct::LParen),
            ) => self.pattern(),
            _ => self.conditional_expression(),
        }
    }

    pub(super) fn pattern(&mut self) -> Parsed {
        if let (
            TokenKind::Keyword(Keyword::Underscore),
            TokenKind::Punct(Punct::Comma | Punct::RParen),
        ) = (self.kind(0), self.kind(1))
        {
            let mut children = Vec::new();
            self.bump(&mut children);
            return Ok(Element::Node(Node {
                kind: NodeKind::MatchAllPattern,
                children,
            }));
        }
        let mut children = Vec::new();
        self.modifiers(&mut children)?;
        children.push(self.ty()?);
        match self.kind(0) {
            TokenKind::Punct(Punct::LParen) => {
                self.bump(&mut children);
                while self.kind(0) != TokenKind::Punct(Punct::RParen) {
                    children.push(self.pattern()?);
                    match self.kind(0) {
                        TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                        _ => break,
                    }
                }
                self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
                Ok(Element::Node(Node {
                    kind: NodeKind::RecordPattern,
                    children,
                }))
            }
            _ => {
                self.expect_identifier(&mut children)?;
                Ok(Element::Node(Node {
                    kind: NodeKind::TypePattern,
                    children,
                }))
            }
        }
    }
}
