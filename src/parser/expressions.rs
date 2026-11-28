use super::{Parsed, Parser, TypeContext};
use crate::lexer::token::{Keyword, Punct, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};

const BINARY: [(Punct, u8); 19] = [
    (Punct::PipePipe, 1),
    (Punct::AmpAmp, 2),
    (Punct::Pipe, 3),
    (Punct::Caret, 4),
    (Punct::Amp, 5),
    (Punct::EqEq, 6),
    (Punct::BangEq, 6),
    (Punct::Lt, 7),
    (Punct::Gt, 7),
    (Punct::LtEq, 7),
    (Punct::GtEq, 7),
    (Punct::LtLt, 8),
    (Punct::GtGt, 8),
    (Punct::GtGtGt, 8),
    (Punct::Plus, 9),
    (Punct::Minus, 9),
    (Punct::Star, 10),
    (Punct::Slash, 10),
    (Punct::Percent, 10),
];

const INSTANCEOF_PRECEDENCE: u8 = 7;

impl Parser<'_> {
    fn lambda_start(&self) -> LambdaStart {
        let close = match (self.kind(0), self.split) {
            (TokenKind::Punct(Punct::LParen), None) => self
                .closing
                .get(self.pos)
                .copied()
                .flatten()
                .map(|position| position - self.pos),
            _ => None,
        };
        match (self.kind(0), self.kind(1), close.map(|n| self.kind(n + 1))) {
            (
                TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore),
                TokenKind::Punct(Punct::Arrow),
                _,
            )
            | (TokenKind::Punct(Punct::LParen), _, Some(TokenKind::Punct(Punct::Arrow))) => {
                LambdaStart::Lambda
            }
            _ => LambdaStart::Other,
        }
    }

    pub(super) fn expression(&mut self) -> Parsed {
        match self.lambda_start() {
            LambdaStart::Lambda => self.lambda(),
            LambdaStart::Other => {
                let target = self.conditional_expression()?;
                match self.kind(0) {
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
                        | Punct::GtGtGtEq,
                    ) => {
                        let mut children = vec![target];
                        self.bump(&mut children);
                        children.push(self.expression()?);
                        Ok(Element::Node(Node {
                            kind: NodeKind::Assignment,
                            children,
                        }))
                    }
                    _ => Ok(target),
                }
            }
        }
    }

    fn lambda(&mut self) -> Parsed {
        let mut children = Vec::new();
        let mut parameters = Vec::new();
        match self.kind(0) {
            TokenKind::Punct(Punct::LParen) => {
                self.bump(&mut parameters);
                while self.kind(0) != TokenKind::Punct(Punct::RParen) {
                    match (self.kind(0), self.kind(1)) {
                        (
                            TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore),
                            TokenKind::Punct(Punct::Comma | Punct::RParen),
                        ) => self.bump(&mut parameters),
                        _ => {
                            let mut parameter = Vec::new();
                            self.modifiers(&mut parameter)?;
                            parameter.push(self.ty()?);
                            if self.kind(0) == TokenKind::Punct(Punct::Ellipsis) {
                                self.bump(&mut parameter);
                            }
                            self.expect_identifier(&mut parameter, Self::fail)?;
                            self.dimensions(&mut parameter)?;
                            parameters.push(Element::Node(Node {
                                kind: NodeKind::FormalParameter,
                                children: parameter,
                            }));
                        }
                    }
                    match self.kind(0) {
                        TokenKind::Punct(Punct::Comma) => self.bump(&mut parameters),
                        _ => break,
                    }
                }
                self.expect(TokenKind::Punct(Punct::RParen), &mut parameters)?;
            }
            _ => self.bump(&mut parameters),
        }
        children.push(Element::Node(Node {
            kind: NodeKind::LambdaParameters,
            children: parameters,
        }));
        self.expect(TokenKind::Punct(Punct::Arrow), &mut children)?;
        match self.kind(0) {
            TokenKind::Punct(Punct::LBrace) => children.push(self.block()?),
            _ => children.push(self.expression()?),
        }
        Ok(Element::Node(Node {
            kind: NodeKind::Lambda,
            children,
        }))
    }

    pub(super) fn conditional_expression(&mut self) -> Parsed {
        let condition = self.binary(1)?;
        match self.kind(0) {
            TokenKind::Punct(Punct::Question) => {
                let mut children = vec![condition];
                self.bump(&mut children);
                children.push(self.expression()?);
                self.expect(TokenKind::Punct(Punct::Colon), &mut children)?;
                match self.lambda_start() {
                    LambdaStart::Lambda => children.push(self.lambda()?),
                    LambdaStart::Other => children.push(self.conditional_expression()?),
                }
                Ok(Element::Node(Node {
                    kind: NodeKind::Conditional,
                    children,
                }))
            }
            _ => Ok(condition),
        }
    }

    fn binary(&mut self, minimum: u8) -> Parsed {
        let mut left = self.unary()?;
        loop {
            let operator = match self.kind(0) {
                TokenKind::Keyword(Keyword::Instanceof) => Some(INSTANCEOF_PRECEDENCE),
                TokenKind::Punct(punct) => BINARY
                    .iter()
                    .find(|(p, _)| *p == punct)
                    .map(|(_, precedence)| *precedence),
                _ => None,
            };
            match (operator, self.kind(0)) {
                (Some(precedence), TokenKind::Keyword(Keyword::Instanceof))
                    if precedence >= minimum =>
                {
                    let mut children = vec![left];
                    self.bump(&mut children);
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
                        ) => {
                            children.push(self.pattern(Self::missing)?);
                        }
                        _ => {
                            self.modifiers(&mut children)?;
                            children.push(self.ty()?);
                        }
                    }
                    left = Element::Node(Node {
                        kind: NodeKind::InstanceOf,
                        children,
                    });
                }
                (Some(precedence), _) if precedence >= minimum => {
                    let mut children = vec![left];
                    self.bump(&mut children);
                    children.push(self.binary(precedence + 1)?);
                    left = Element::Node(Node {
                        kind: NodeKind::Binary,
                        children,
                    });
                }
                _ => return Ok(left),
            }
        }
    }

    fn unary(&mut self) -> Parsed {
        match self.kind(0) {
            TokenKind::Punct(
                Punct::Plus
                | Punct::Minus
                | Punct::PlusPlus
                | Punct::MinusMinus
                | Punct::Bang
                | Punct::Tilde,
            ) => {
                let mut children = Vec::new();
                self.bump(&mut children);
                children.push(self.unary()?);
                Ok(Element::Node(Node {
                    kind: NodeKind::Unary,
                    children,
                }))
            }
            TokenKind::Punct(Punct::LParen) => {
                let cast = self.probe(|p| {
                    let mut ignored = Vec::new();
                    p.bump(&mut ignored);
                    let primitive = matches!(
                        p.kind(0),
                        TokenKind::Keyword(
                            Keyword::Boolean
                                | Keyword::Byte
                                | Keyword::Short
                                | Keyword::Int
                                | Keyword::Long
                                | Keyword::Char
                                | Keyword::Float
                                | Keyword::Double
                        )
                    );
                    p.ty()?;
                    while p.kind(0) == TokenKind::Punct(Punct::Amp) {
                        p.bump(&mut ignored);
                        p.ty()?;
                    }
                    p.expect(TokenKind::Punct(Punct::RParen), &mut ignored)?;
                    Ok((primitive, operand_start(p.kind(0))))
                });
                match cast {
                    Ok((true, OperandStart::Signed | OperandStart::Unsigned))
                    | Ok((false, OperandStart::Unsigned)) => self.cast(),
                    _ => self.postfix(),
                }
            }
            _ => self.postfix(),
        }
    }

    fn cast(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        children.push(self.ty()?);
        while self.kind(0) == TokenKind::Punct(Punct::Amp) {
            self.bump(&mut children);
            children.push(self.ty()?);
        }
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        let operand = match self.lambda_start() {
            LambdaStart::Lambda => self.lambda()?,
            LambdaStart::Other => self.unary()?,
        };
        children.push(operand);
        Ok(Element::Node(Node {
            kind: NodeKind::Cast,
            children,
        }))
    }

    fn postfix(&mut self) -> Parsed {
        let mut operand = self.primary()?;
        while let TokenKind::Punct(Punct::PlusPlus | Punct::MinusMinus) = self.kind(0) {
            let mut children = vec![operand];
            self.bump(&mut children);
            operand = Element::Node(Node {
                kind: NodeKind::Postfix,
                children,
            });
        }
        Ok(operand)
    }

    pub(super) fn arguments(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
        while self.kind(0) != TokenKind::Punct(Punct::RParen) {
            children.push(self.expression()?);
            match self.kind(0) {
                TokenKind::Punct(Punct::Comma) => self.bump(&mut children),
                _ => break,
            }
        }
        self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::Arguments,
            children,
        }))
    }

    fn primary(&mut self) -> Parsed {
        let mut children = Vec::new();
        let base = match self.kind(0) {
            TokenKind::IntegerLiteral
            | TokenKind::FloatingLiteral
            | TokenKind::CharLiteral
            | TokenKind::StringLiteral
            | TokenKind::TextBlock
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Null => {
                self.bump(&mut children);
                Element::Node(Node {
                    kind: NodeKind::Literal,
                    children,
                })
            }
            TokenKind::Punct(Punct::LParen) => {
                self.bump(&mut children);
                children.push(self.expression()?);
                self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
                Element::Node(Node {
                    kind: NodeKind::Parenthesized,
                    children,
                })
            }
            TokenKind::Keyword(Keyword::This) => {
                self.bump(&mut children);
                Element::Node(Node {
                    kind: NodeKind::This,
                    children,
                })
            }
            TokenKind::Keyword(Keyword::Super) => {
                self.bump(&mut children);
                Element::Node(Node {
                    kind: NodeKind::Super,
                    children,
                })
            }
            TokenKind::Keyword(Keyword::New) => self.creator(Vec::new())?,
            TokenKind::Punct(Punct::At) => self.ty()?,
            TokenKind::Keyword(Keyword::Switch) => {
                self.bump(&mut children);
                self.expect(TokenKind::Punct(Punct::LParen), &mut children)?;
                children.push(self.expression()?);
                self.expect(TokenKind::Punct(Punct::RParen), &mut children)?;
                children.push(self.switch_block()?);
                Element::Node(Node {
                    kind: NodeKind::SwitchExpr,
                    children,
                })
            }
            TokenKind::Keyword(Keyword::Void) => {
                self.bump(&mut children);
                Element::Node(Node {
                    kind: NodeKind::VoidType,
                    children,
                })
            }
            TokenKind::Keyword(
                Keyword::Boolean
                | Keyword::Byte
                | Keyword::Short
                | Keyword::Int
                | Keyword::Long
                | Keyword::Char
                | Keyword::Float
                | Keyword::Double,
            ) => self.ty()?,
            TokenKind::Punct(Punct::Lt) => {
                children.push(self.type_arguments()?);
                match self.kind(0) {
                    TokenKind::Keyword(Keyword::This | Keyword::Super) => self.bump(&mut children),
                    _ => return self.fail("<identifier> expected"),
                }
                children.push(self.arguments()?);
                Element::Node(Node {
                    kind: NodeKind::MethodCall,
                    children,
                })
            }
            TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore) => {
                let typed = self.probe(|p| {
                    p.ty()?;
                    Ok((p.kind(0), p.kind(1)))
                });
                match typed {
                    Ok((TokenKind::Punct(Punct::ColonColon), _))
                    | Ok((TokenKind::Punct(Punct::Dot), TokenKind::Keyword(Keyword::Class))) => {
                        self.ty()?
                    }
                    _ => {
                        self.bump(&mut children);
                        Element::Node(Node {
                            kind: NodeKind::Name,
                            children,
                        })
                    }
                }
            }
            _ => return self.fail("illegal start of expression"),
        };
        self.selectors(base)
    }

    fn creator(&mut self, mut children: Vec<Element>) -> Parsed {
        self.bump(&mut children);
        if self.kind(0) == TokenKind::Punct(Punct::Lt) {
            children.push(self.type_arguments()?);
        }
        let created = self.ty()?;
        let array = matches!(&created, Element::Node(node) if node.kind == NodeKind::ArrayType);
        children.push(created);
        match (self.kind(0), array) {
            (TokenKind::Punct(Punct::LBracket | Punct::At), _) => {
                loop {
                    let sized = self.probe(|p| {
                        while p.kind(0) == TokenKind::Punct(Punct::At) {
                            p.annotation()?;
                        }
                        Ok((p.kind(0), p.kind(1)))
                    });
                    match sized {
                        Ok((TokenKind::Punct(Punct::LBracket), next))
                            if next != TokenKind::Punct(Punct::RBracket) => {}
                        _ => break,
                    }
                    let mut dimension = Vec::new();
                    while self.kind(0) == TokenKind::Punct(Punct::At) {
                        dimension.push(self.annotation()?);
                    }
                    self.bump(&mut dimension);
                    dimension.push(self.expression()?);
                    self.expect(TokenKind::Punct(Punct::RBracket), &mut dimension)?;
                    children.push(Element::Node(Node {
                        kind: NodeKind::Dimension,
                        children: dimension,
                    }));
                }
                self.dimensions(&mut children)?;
                Ok(Element::Node(Node {
                    kind: NodeKind::NewArray,
                    children,
                }))
            }
            (TokenKind::Punct(Punct::LBrace), true) => {
                children.push(self.array_initializer()?);
                Ok(Element::Node(Node {
                    kind: NodeKind::NewArray,
                    children,
                }))
            }
            _ => {
                children.push(self.arguments()?);
                if self.kind(0) == TokenKind::Punct(Punct::LBrace) {
                    children.push(self.class_body(TypeContext::Other)?);
                }
                Ok(Element::Node(Node {
                    kind: NodeKind::NewClass,
                    children,
                }))
            }
        }
    }

    fn selectors(&mut self, mut base: Element) -> Parsed {
        loop {
            let callable = matches!(&base, Element::Node(node) if matches!(node.kind, NodeKind::Name | NodeKind::This | NodeKind::Super));
            base = match (self.kind(0), self.kind(1), callable) {
                (TokenKind::Punct(Punct::LParen), _, true) => {
                    let mut children = vec![base];
                    children.push(self.arguments()?);
                    Element::Node(Node {
                        kind: NodeKind::MethodCall,
                        children,
                    })
                }
                (TokenKind::Punct(Punct::Dot), TokenKind::Keyword(Keyword::New), _) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    self.creator(children)?
                }
                (TokenKind::Punct(Punct::Dot), TokenKind::Keyword(Keyword::Class), _) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    self.bump(&mut children);
                    Element::Node(Node {
                        kind: NodeKind::ClassLiteral,
                        children,
                    })
                }
                (
                    TokenKind::Punct(Punct::Dot),
                    TokenKind::Keyword(Keyword::This | Keyword::Super),
                    _,
                ) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    let kind = match self.kind(0) {
                        TokenKind::Keyword(Keyword::This) => NodeKind::This,
                        _ => NodeKind::Super,
                    };
                    self.bump(&mut children);
                    Element::Node(Node { kind, children })
                }
                (TokenKind::Punct(Punct::Dot), _, _) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    if self.kind(0) == TokenKind::Punct(Punct::Lt) {
                        children.push(self.type_arguments()?);
                    }
                    self.expect_identifier(&mut children, Self::missing)?;
                    match self.kind(0) {
                        TokenKind::Punct(Punct::LParen) => {
                            children.push(self.arguments()?);
                            Element::Node(Node {
                                kind: NodeKind::MethodCall,
                                children,
                            })
                        }
                        _ => Element::Node(Node {
                            kind: NodeKind::FieldAccess,
                            children,
                        }),
                    }
                }
                (TokenKind::Punct(Punct::LBracket), _, _) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    children.push(self.expression()?);
                    self.expect(TokenKind::Punct(Punct::RBracket), &mut children)?;
                    Element::Node(Node {
                        kind: NodeKind::ArrayAccess,
                        children,
                    })
                }
                (TokenKind::Punct(Punct::ColonColon), _, _) => {
                    let mut children = vec![base];
                    self.bump(&mut children);
                    if self.kind(0) == TokenKind::Punct(Punct::Lt) {
                        children.push(self.type_arguments()?);
                    }
                    match self.kind(0) {
                        TokenKind::Keyword(Keyword::New) => self.bump(&mut children),
                        _ => self.expect_identifier(&mut children, Self::missing)?,
                    }
                    Element::Node(Node {
                        kind: NodeKind::MethodReference,
                        children,
                    })
                }
                _ => return Ok(base),
            };
        }
    }
}

enum LambdaStart {
    Lambda,
    Other,
}

enum OperandStart {
    Signed,
    Unsigned,
    Other,
}

fn operand_start(kind: TokenKind) -> OperandStart {
    match kind {
        TokenKind::Punct(Punct::Plus | Punct::Minus | Punct::PlusPlus | Punct::MinusMinus) => {
            OperandStart::Signed
        }
        TokenKind::Identifier
        | TokenKind::IntegerLiteral
        | TokenKind::FloatingLiteral
        | TokenKind::CharLiteral
        | TokenKind::StringLiteral
        | TokenKind::TextBlock
        | TokenKind::True
        | TokenKind::False
        | TokenKind::Null
        | TokenKind::Punct(Punct::LParen | Punct::Bang | Punct::Tilde)
        | TokenKind::Keyword(
            Keyword::This
            | Keyword::Super
            | Keyword::New
            | Keyword::Switch
            | Keyword::Void
            | Keyword::Underscore
            | Keyword::Boolean
            | Keyword::Byte
            | Keyword::Short
            | Keyword::Int
            | Keyword::Long
            | Keyword::Char
            | Keyword::Float
            | Keyword::Double,
        ) => OperandStart::Unsigned,
        _ => OperandStart::Other,
    }
}
