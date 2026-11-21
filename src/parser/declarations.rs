use super::{Parsed, Parser, TypeContext};

pub(super) enum Modified {
    No,
    Yes,
}
use crate::lexer::token::{Keyword, Punct, TokenKind};
use crate::syntax::{Element, Node, NodeKind};

impl Parser<'_> {
    pub(super) fn modifiers(
        &mut self,
        out: &mut Vec<Element>,
    ) -> Result<Modified, super::ParseError> {
        let continues_declaration = |next: TokenKind, text: &str| {
            matches!(
                next,
                TokenKind::Keyword(
                    Keyword::Class
                        | Keyword::Interface
                        | Keyword::Abstract
                        | Keyword::Final
                        | Keyword::Static
                        | Keyword::Strictfp
                        | Keyword::Public
                        | Keyword::Protected
                        | Keyword::Private
                ) | TokenKind::Punct(Punct::At)
            ) || (next == TokenKind::Identifier && matches!(text, "sealed" | "non" | "record"))
        };
        let mut children = Vec::new();
        loop {
            match (self.kind(0), self.text(0), self.kind(1), self.text(1)) {
                (TokenKind::Punct(Punct::At), _, TokenKind::Keyword(Keyword::Interface), _) => {
                    break;
                }
                (TokenKind::Punct(Punct::At), _, _, _) => children.push(self.annotation()?),
                (
                    TokenKind::Keyword(
                        Keyword::Public
                        | Keyword::Protected
                        | Keyword::Private
                        | Keyword::Static
                        | Keyword::Abstract
                        | Keyword::Final
                        | Keyword::Native
                        | Keyword::Synchronized
                        | Keyword::Transient
                        | Keyword::Volatile
                        | Keyword::Strictfp,
                    ),
                    _,
                    _,
                    _,
                ) => self.bump(&mut children),
                (TokenKind::Keyword(Keyword::Default), _, next, _)
                    if !matches!(next, TokenKind::Punct(Punct::Colon | Punct::Arrow)) =>
                {
                    self.bump(&mut children);
                }
                (TokenKind::Identifier, "sealed", next, next_text)
                    if continues_declaration(next, next_text) =>
                {
                    self.bump(&mut children);
                }
                (TokenKind::Identifier, "non", TokenKind::Punct(Punct::Minus), "-") => {
                    match (self.text(2), self.kind(3), self.text(3)) {
                        ("sealed", next, next_text) if continues_declaration(next, next_text) => {
                            self.bump(&mut children);
                            self.bump(&mut children);
                            self.bump(&mut children);
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        let modified = match children.is_empty() {
            true => Modified::No,
            false => Modified::Yes,
        };
        out.push(Element::Node(Node {
            kind: NodeKind::Modifiers,
            children,
        }));
        Ok(modified)
    }

    pub(super) fn annotation(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        children.push(self.qualified_name()?);
        if self.kind(0) == TokenKind::Punct(Punct::LParen) {
            let mut arguments = Vec::new();
            self.bump(&mut arguments);
            while self.kind(0) != TokenKind::Punct(Punct::RParen) {
                match (self.kind(0), self.kind(1)) {
                    (TokenKind::Identifier, TokenKind::Punct(Punct::Assign)) => {
                        let mut pair = Vec::new();
                        self.bump(&mut pair);
                        self.bump(&mut pair);
                        pair.push(self.element_value()?);
                        arguments.push(Element::Node(Node {
                            kind: NodeKind::ElementValuePair,
                            children: pair,
                        }));
                    }
                    _ => arguments.push(self.element_value()?),
                }
                match self.kind(0) {
                    TokenKind::Punct(Punct::Comma) => self.bump(&mut arguments),
                    _ => break,
                }
            }
            self.expect(TokenKind::Punct(Punct::RParen), &mut arguments)?;
            children.push(Element::Node(Node {
                kind: NodeKind::AnnotationArguments,
                children: arguments,
            }));
        }
        Ok(Element::Node(Node {
            kind: NodeKind::Annotation,
            children,
        }))
    }

    pub(super) fn element_value(&mut self) -> Parsed {
        match self.kind(0) {
            TokenKind::Punct(Punct::At) => self.annotation(),
            TokenKind::Punct(Punct::LBrace) => {
                let mut children = Vec::new();
                self.bump(&mut children);
                loop {
                    match self.kind(0) {
                        TokenKind::Punct(Punct::RBrace) => break,
                        TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                        _ => children.push(self.element_value()?),
                    }
                }
                self.bump(&mut children);
                Ok(Element::Node(Node {
                    kind: NodeKind::ElementValueArray,
                    children,
                }))
            }
            _ => self.conditional_expression(),
        }
    }

    pub(super) fn member(&mut self, context: TypeContext) -> Parsed {
        let mut children = Vec::new();
        if self.kind(0) == TokenKind::Punct(Punct::Semicolon) {
            self.bump(&mut children);
            return Ok(Element::Node(Node {
                kind: NodeKind::EmptyDecl,
                children,
            }));
        }
        self.modifiers(&mut children)?;
        match (self.kind(0), self.text(0), self.kind(1), self.kind(2)) {
            (TokenKind::Punct(Punct::LBrace), _, _, _) => {
                children.push(self.block()?);
                Ok(Element::Node(Node {
                    kind: NodeKind::Initializer,
                    children,
                }))
            }
            (
                TokenKind::Keyword(Keyword::Class | Keyword::Interface | Keyword::Enum)
                | TokenKind::Punct(Punct::At),
                _,
                _,
                _,
            ) => self.type_declaration(children),
            (
                TokenKind::Identifier,
                "record",
                TokenKind::Identifier,
                TokenKind::Punct(Punct::LParen | Punct::Lt),
            ) => self.type_declaration(children),
            (TokenKind::Identifier, _, TokenKind::Punct(Punct::LBrace), _)
                if context == TypeContext::Record =>
            {
                self.bump(&mut children);
                children.push(self.block()?);
                Ok(Element::Node(Node {
                    kind: NodeKind::CompactConstructorDecl,
                    children,
                }))
            }
            _ => {
                if self.kind(0) == TokenKind::Punct(Punct::Lt) {
                    children.push(self.type_parameters()?);
                }
                match (self.kind(0), self.kind(1)) {
                    (TokenKind::Identifier, TokenKind::Punct(Punct::LParen)) => {
                        self.bump(&mut children);
                        self.method_rest(children, NodeKind::ConstructorDecl)
                    }
                    (TokenKind::Keyword(Keyword::Void), _) => {
                        let mut void = Vec::new();
                        self.bump(&mut void);
                        children.push(Element::Node(Node {
                            kind: NodeKind::VoidType,
                            children: void,
                        }));
                        self.expect_identifier(&mut children)?;
                        self.method_rest(children, NodeKind::MethodDecl)
                    }
                    _ => {
                        children.push(self.ty()?);
                        match (self.kind(0), self.kind(1)) {
                            (TokenKind::Identifier, TokenKind::Punct(Punct::LParen)) => {
                                self.bump(&mut children);
                                self.method_rest(children, NodeKind::MethodDecl)
                            }
                            _ => {
                                self.variable_declarators(&mut children)?;
                                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
                                Ok(Element::Node(Node {
                                    kind: NodeKind::FieldDecl,
                                    children,
                                }))
                            }
                        }
                    }
                }
            }
        }
    }

    fn method_rest(&mut self, mut children: Vec<Element>, kind: NodeKind) -> Parsed {
        children.push(self.formal_parameters()?);
        self.dimensions(&mut children)?;
        if self.kind(0) == TokenKind::Keyword(Keyword::Throws) {
            let mut throws = Vec::new();
            self.bump(&mut throws);
            throws.push(self.ty()?);
            while self.kind(0) == TokenKind::Punct(Punct::Comma) {
                self.bump(&mut throws);
                throws.push(self.ty()?);
            }
            children.push(Element::Node(Node {
                kind: NodeKind::ThrowsClause,
                children: throws,
            }));
        }
        match self.kind(0) {
            TokenKind::Punct(Punct::LBrace) => children.push(self.block()?),
            TokenKind::Keyword(Keyword::Default) => {
                let mut default = Vec::new();
                self.bump(&mut default);
                default.push(self.element_value()?);
                children.push(Element::Node(Node {
                    kind: NodeKind::DefaultValue,
                    children: default,
                }));
                self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
            }
            _ => self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?,
        }
        Ok(Element::Node(Node { kind, children }))
    }

    pub(super) fn variable_declarators(
        &mut self,
        out: &mut Vec<Element>,
    ) -> Result<(), super::ParseError> {
        loop {
            let mut declarator = Vec::new();
            self.expect_identifier(&mut declarator)?;
            self.dimensions(&mut declarator)?;
            if self.kind(0) == TokenKind::Punct(Punct::Assign) {
                self.bump(&mut declarator);
                declarator.push(self.variable_initializer()?);
            }
            out.push(Element::Node(Node {
                kind: NodeKind::VariableDeclarator,
                children: declarator,
            }));
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(out),
                _ => return Ok(()),
            }
        }
    }

    pub(super) fn variable_initializer(&mut self) -> Parsed {
        match self.kind(0) {
            TokenKind::Punct(Punct::LBrace) => self.array_initializer(),
            _ => self.expression(),
        }
    }

    pub(super) fn array_initializer(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        loop {
            match self.kind(0) {
                TokenKind::Punct(Punct::RBrace) => break,
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                _ => children.push(self.variable_initializer()?),
            }
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::ArrayInitializer,
            children,
        }))
    }

    pub(super) fn type_declaration(&mut self, mut children: Vec<Element>) -> Parsed {
        let (kind, context) = match (self.kind(0), self.text(0)) {
            (TokenKind::Keyword(Keyword::Class), _) => (NodeKind::ClassDecl, TypeContext::Other),
            (TokenKind::Keyword(Keyword::Interface), _) => {
                (NodeKind::InterfaceDecl, TypeContext::Other)
            }
            (TokenKind::Keyword(Keyword::Enum), _) => (NodeKind::EnumDecl, TypeContext::Other),
            (TokenKind::Punct(Punct::At), _) => {
                self.bump(&mut children);
                (NodeKind::AnnotationTypeDecl, TypeContext::Other)
            }
            (TokenKind::Identifier, "record") => (NodeKind::RecordDecl, TypeContext::Record),
            _ => return self.fail("class, interface, enum, or record expected"),
        };
        self.bump(&mut children);
        self.expect_identifier(&mut children)?;
        if self.kind(0) == TokenKind::Punct(Punct::Lt) {
            children.push(self.type_parameters()?);
        }
        if kind == NodeKind::RecordDecl {
            children.push(self.record_header()?);
        }
        loop {
            let clause = match (self.kind(0), self.text(0)) {
                (TokenKind::Keyword(Keyword::Extends), _) => NodeKind::ExtendsClause,
                (TokenKind::Keyword(Keyword::Implements), _) => NodeKind::ImplementsClause,
                (TokenKind::Identifier, "permits") => NodeKind::PermitsClause,
                _ => break,
            };
            let mut types = Vec::new();
            self.bump(&mut types);
            types.push(self.ty()?);
            while self.kind(0) == TokenKind::Punct(Punct::Comma) {
                self.bump(&mut types);
                types.push(self.ty()?);
            }
            children.push(Element::Node(Node {
                kind: clause,
                children: types,
            }));
        }
        match kind {
            NodeKind::EnumDecl => children.push(self.enum_body()?),
            _ => children.push(self.class_body(context)?),
        }
        Ok(Element::Node(Node { kind, children }))
    }

    pub(super) fn class_body(&mut self, context: TypeContext) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LBrace), &mut children)?;
        loop {
            match self.kind(0) {
                TokenKind::Punct(Punct::RBrace) => break,
                TokenKind::Eof => return self.fail("'}' expected"),
                _ => children.push(self.member(context)?),
            }
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::ClassBody,
            children,
        }))
    }

    fn enum_body(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LBrace), &mut children)?;
        loop {
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                TokenKind::Punct(Punct::Semicolon | Punct::RBrace) => break,
                _ => {
                    let mut constant = Vec::new();
                    self.modifiers(&mut constant)?;
                    self.expect_identifier(&mut constant)?;
                    if self.kind(0) == TokenKind::Punct(Punct::LParen) {
                        constant.push(self.arguments()?);
                    }
                    if self.kind(0) == TokenKind::Punct(Punct::LBrace) {
                        constant.push(self.class_body(TypeContext::Other)?);
                    }
                    children.push(Element::Node(Node {
                        kind: NodeKind::EnumConstant,
                        children: constant,
                    }));
                }
            }
        }
        if self.kind(0) == TokenKind::Punct(Punct::Semicolon) {
            self.bump(&mut children);
            loop {
                match self.kind(0) {
                    TokenKind::Punct(Punct::RBrace) => break,
                    TokenKind::Eof => return self.fail("'}' expected"),
                    _ => children.push(self.member(TypeContext::Other)?),
                }
            }
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::EnumBody,
            children,
        }))
    }

    fn record_header(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
        while self.kind(0) != TokenKind::Punct(Punct::RParen) {
            let mut component = Vec::new();
            self.modifiers(&mut component)?;
            component.push(self.ty()?);
            if self.kind(0) == TokenKind::Punct(Punct::Ellipsis) {
                self.bump(&mut component);
            }
            self.expect_identifier(&mut component)?;
            children.push(Element::Node(Node {
                kind: NodeKind::RecordComponent,
                children: component,
            }));
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                _ => break,
            }
        }
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::RecordHeader,
            children,
        }))
    }

    pub(super) fn type_parameters(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        loop {
            let mut parameter = Vec::new();
            self.modifiers(&mut parameter)?;
            self.expect_identifier(&mut parameter)?;
            if self.kind(0) == TokenKind::Keyword(Keyword::Extends) {
                let mut bound = Vec::new();
                self.bump(&mut bound);
                bound.push(self.ty()?);
                while self.kind(0) == TokenKind::Punct(Punct::Amp) {
                    self.bump(&mut bound);
                    bound.push(self.ty()?);
                }
                parameter.push(Element::Node(Node {
                    kind: NodeKind::TypeBound,
                    children: bound,
                }));
            }
            children.push(Element::Node(Node {
                kind: NodeKind::TypeParameter,
                children: parameter,
            }));
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                _ => break,
            }
        }
        self.close_angle(&mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::TypeParameters,
            children,
        }))
    }

    fn formal_parameters(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
        while self.kind(0) != TokenKind::Punct(Punct::RParen) {
            let mut parameter = Vec::new();
            self.modifiers(&mut parameter)?;
            parameter.push(self.ty()?);
            let kind = match (self.kind(0), self.kind(1), self.kind(2)) {
                (TokenKind::Keyword(Keyword::This), _, _) => {
                    self.bump(&mut parameter);
                    NodeKind::ReceiverParameter
                }
                (
                    TokenKind::Identifier,
                    TokenKind::Punct(Punct::Dot),
                    TokenKind::Keyword(Keyword::This),
                ) => {
                    self.bump(&mut parameter);
                    self.bump(&mut parameter);
                    self.bump(&mut parameter);
                    NodeKind::ReceiverParameter
                }
                _ => {
                    while self.kind(0) == TokenKind::Punct(Punct::At) {
                        parameter.push(self.annotation()?);
                    }
                    if self.kind(0) == TokenKind::Punct(Punct::Ellipsis) {
                        self.bump(&mut parameter);
                    }
                    self.expect_identifier(&mut parameter)?;
                    self.dimensions(&mut parameter)?;
                    NodeKind::FormalParameter
                }
            };
            children.push(Element::Node(Node {
                kind,
                children: parameter,
            }));
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                _ => break,
            }
        }
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::FormalParameters,
            children,
        }))
    }
}
