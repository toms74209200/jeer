use super::{ParseError, Parsed, Parser};
use crate::lexer::token::{Keyword, Punct, TokenKind};
use crate::syntax::{Element, Node, NodeKind};

impl Parser<'_> {
    pub(super) fn ty(&mut self) -> Parsed {
        let mut annotations = Vec::new();
        while let (TokenKind::Punct(Punct::At), false) = (
            self.kind(0),
            self.kind(1) == TokenKind::Keyword(Keyword::Interface),
        ) {
            annotations.push(self.annotation()?);
        }
        let base = match self.kind(0) {
            TokenKind::Keyword(
                Keyword::Boolean
                | Keyword::Byte
                | Keyword::Short
                | Keyword::Int
                | Keyword::Long
                | Keyword::Char
                | Keyword::Float
                | Keyword::Double,
            ) => {
                let mut children = annotations;
                self.bump(&mut children);
                Element::Node(Node {
                    kind: NodeKind::PrimitiveType,
                    children,
                })
            }
            TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore) => {
                let mut children = annotations;
                self.bump(&mut children);
                loop {
                    if self.kind(0) == TokenKind::Punct(Punct::Lt) {
                        children.push(self.type_arguments()?);
                    }
                    let annotated = self.probe(|p| {
                        let mut ignored = Vec::new();
                        p.bump(&mut ignored);
                        while p.kind(0) == TokenKind::Punct(Punct::At) {
                            p.annotation()?;
                        }
                        Ok(p.kind(0))
                    });
                    match (self.kind(0), annotated) {
                        (TokenKind::Punct(Punct::Dot), Ok(TokenKind::Identifier)) => {
                            self.bump(&mut children);
                            while self.kind(0) == TokenKind::Punct(Punct::At) {
                                children.push(self.annotation()?);
                            }
                            self.bump(&mut children);
                        }
                        _ => break,
                    }
                }
                Element::Node(Node {
                    kind: NodeKind::ClassType,
                    children,
                })
            }
            _ => return self.fail("<type> expected"),
        };
        self.array_type(base)
    }

    pub(super) fn array_type(&mut self, base: Element) -> Parsed {
        let mut dimensions = Vec::new();
        self.dimensions(&mut dimensions)?;
        match dimensions.is_empty() {
            true => Ok(base),
            false => {
                let mut children = vec![base];
                children.extend(dimensions);
                Ok(Element::Node(Node {
                    kind: NodeKind::ArrayType,
                    children,
                }))
            }
        }
    }

    pub(super) fn dimensions(&mut self, out: &mut Vec<Element>) -> Result<(), ParseError> {
        loop {
            let bracket = self.probe(|p| {
                let mut ignored = Vec::new();
                while p.kind(0) == TokenKind::Punct(Punct::At) {
                    p.annotation()?;
                }
                p.bump(&mut ignored);
                Ok((ignored.last().cloned(), p.kind(0)))
            });
            match bracket {
                Ok((Some(Element::Token(open)), TokenKind::Punct(Punct::RBracket)))
                    if open.kind == TokenKind::Punct(Punct::LBracket) =>
                {
                    let mut dimension = Vec::new();
                    while self.kind(0) == TokenKind::Punct(Punct::At) {
                        dimension.push(self.annotation()?);
                    }
                    self.bump(&mut dimension);
                    self.bump(&mut dimension);
                    out.push(Element::Node(Node {
                        kind: NodeKind::Dimension,
                        children: dimension,
                    }));
                }
                _ => return Ok(()),
            }
        }
    }

    pub(super) fn type_arguments(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        if !matches!(
            self.kind(0),
            TokenKind::Punct(
                Punct::Gt
                    | Punct::GtGt
                    | Punct::GtGtGt
                    | Punct::GtEq
                    | Punct::GtGtEq
                    | Punct::GtGtGtEq
            )
        ) {
            loop {
                let wildcard = self.probe(|p| {
                    let mut ignored = Vec::new();
                    while p.kind(0) == TokenKind::Punct(Punct::At) {
                        p.annotation()?;
                    }
                    p.bump(&mut ignored);
                    Ok(ignored.last().cloned())
                });
                match wildcard {
                    Ok(Some(Element::Token(question)))
                        if question.kind == TokenKind::Punct(Punct::Question) =>
                    {
                        let mut argument = Vec::new();
                        while self.kind(0) == TokenKind::Punct(Punct::At) {
                            argument.push(self.annotation()?);
                        }
                        self.bump(&mut argument);
                        if let TokenKind::Keyword(Keyword::Extends | Keyword::Super) = self.kind(0)
                        {
                            self.bump(&mut argument);
                            argument.push(self.ty()?);
                        }
                        children.push(Element::Node(Node {
                            kind: NodeKind::Wildcard,
                            children: argument,
                        }));
                    }
                    _ => children.push(self.ty()?),
                }
                match self.kind(0) {
                    TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                    _ => break,
                }
            }
        }
        self.close_angle(&mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::TypeArguments,
            children,
        }))
    }
}
