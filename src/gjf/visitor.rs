mod declarations;
mod expressions;
mod statements;

use super::builder::{Builder, Op, Unformatted};
use super::doc::{BreakMode, Indent};
use super::output::BlankLine;
use crate::lexer::token::{Token, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};
use statements::Collapse;

const ZERO: Indent = Indent::Const(0);
const PLUS_TWO: Indent = Indent::Const(2);
const PLUS_FOUR: Indent = Indent::Const(4);
const MINUS_TWO: Indent = Indent::Const(-2);
const MINUS_FOUR: Indent = Indent::Const(-4);

const OPEN_ZERO: Op<'static> = Op::Open(ZERO);
const OPEN_PLUS_TWO: Op<'static> = Op::Open(PLUS_TWO);
const OPEN_PLUS_FOUR: Op<'static> = Op::Open(PLUS_FOUR);
const CLOSE: Op<'static> = Op::Close;
const SPACE: Op<'static> = Op::Space;
const BREAK: Op<'static> = Op::Break {
    mode: BreakMode::Unified,
    flat: "",
    plus_indent: ZERO,
    tag: None,
};
const SPACE_BREAK: Op<'static> = Op::Break {
    mode: BreakMode::Unified,
    flat: " ",
    plus_indent: ZERO,
    tag: None,
};
const FILL: Op<'static> = Op::Break {
    mode: BreakMode::Fill,
    flat: "",
    plus_indent: ZERO,
    tag: None,
};
const FILL_SPACE: Op<'static> = Op::Break {
    mode: BreakMode::Fill,
    flat: " ",
    plus_indent: ZERO,
    tag: None,
};
const FORCED: Op<'static> = Op::Break {
    mode: BreakMode::Forced,
    flat: "",
    plus_indent: ZERO,
    tag: None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Vertical,
    Horizontal,
}

pub struct Visitor<'a> {
    pub builder: Builder<'a>,
    pub source: &'a str,
    pub decoded: &'a crate::lexer::unicode_escapes::Translated,
    pub hug_points: std::collections::HashMap<usize, super::doc::HugPoint>,
    pub not_huggable: std::collections::HashSet<usize>,
    pub type_annotations: std::collections::HashSet<String>,
    pub markdown_positions: std::collections::HashSet<usize>,
}

impl<'a> Visitor<'a> {
    fn token(
        &mut self,
        token: &Token,
        comments_indent: Indent,
        trailing: Option<Indent>,
    ) -> Result<(), Unformatted> {
        let text = &self.source[token.start..token.end];
        let decoded = self.decoded.decoded(token.start, token.end);
        match token.kind {
            TokenKind::Punct(_) => {
                for c in decoded.chars() {
                    self.builder.add(Op::Token {
                        text: &c.to_string(),
                        comments_indent: comments_indent.clone(),
                        trailing: trailing.clone(),
                    })?;
                }
                Ok(())
            }
            TokenKind::Identifier
            | TokenKind::Keyword(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Null => self.builder.add(Op::Token {
                text: &decoded,
                comments_indent,
                trailing,
            }),
            TokenKind::IntegerLiteral
            | TokenKind::FloatingLiteral
            | TokenKind::CharLiteral
            | TokenKind::StringLiteral => self.builder.add(Op::Token {
                text,
                comments_indent,
                trailing,
            }),
            TokenKind::TextBlock => {
                let java = |c: char| {
                    (c.is_whitespace()
                        && !matches!(c, '\u{85}' | '\u{a0}' | '\u{2007}' | '\u{202f}'))
                        || ('\u{1c}'..='\u{1f}').contains(&c)
                };
                let initial: Vec<&str> = text.lines().collect();
                let body = &initial[1..];
                let blank = |line: &&str| line.chars().all(java);
                let leading = |line: &&str| line.chars().take_while(|c| java(*c)).count();
                let outdent = body
                    .iter()
                    .filter(|line| !blank(line))
                    .map(leading)
                    .fold(usize::MAX, usize::min);
                let stripped: Vec<String> = body
                    .iter()
                    .map(|line| match blank(line) {
                        true => String::new(),
                        false => line
                            .chars()
                            .skip(outdent)
                            .collect::<String>()
                            .trim_end_matches(java)
                            .to_owned(),
                    })
                    .collect();
                let inner = &stripped[..stripped.len() - 1];
                let last = stripped[stripped.len() - 1].as_str();
                let source_last = initial[initial.len() - 1].trim_end_matches(java);
                if source_last.chars().count() == last.chars().count() {
                    self.builder.add(Op::Break {
                        mode: BreakMode::Unified,
                        flat: "",
                        plus_indent: Indent::Const(i32::MIN),
                        tag: None,
                    })?;
                }
                let delimiter = "\"\"\"";
                let without = last[..last.len() - delimiter.len()].trim_end_matches(java);
                let lines = std::iter::once(initial[0].trim_end_matches(java).to_owned())
                    .chain(inner.iter().cloned())
                    .chain((!without.is_empty()).then(|| format!("{without}\\")))
                    .chain(std::iter::once(delimiter.to_owned()))
                    .collect();
                self.builder.add(Op::Lines { text, lines })
            }
            TokenKind::Whitespace
            | TokenKind::Newline
            | TokenKind::LineComment
            | TokenKind::BlockComment
            | TokenKind::JavadocComment
            | TokenKind::Eof => Err(Unformatted::Deferred),
        }
    }

    fn span(&self, node: &Node) -> Result<(usize, usize), Unformatted> {
        let mut first = node;
        let start = loop {
            match first.children.first() {
                Some(Element::Token(token)) => break token.start,
                Some(Element::Node(child)) => first = child,
                None => return Err(Unformatted::Deferred),
            }
        };
        let mut last = node;
        let end = loop {
            match last.children.last() {
                Some(Element::Token(token)) => break token.end,
                Some(Element::Node(child)) => last = child,
                None => return Err(Unformatted::Deferred),
            }
        };
        Ok((start, end))
    }

    fn actual_size(&self, node: &Node) -> Result<usize, Unformatted> {
        let (start, end) = self.span(node)?;
        let tokens = &self.builder.input.tokens;
        let first = tokens.partition_point(|token| token.tok.position <= start);
        let last = tokens.partition_point(|token| token.tok.position < end);
        let (Some(first), Some(last)) = (
            first.checked_sub(1).and_then(|i| tokens.get(i)),
            last.checked_sub(1).and_then(|i| tokens.get(i)),
        ) else {
            return Err(Unformatted::Deferred);
        };
        let comment =
            |tok: &&super::input::Tok| matches!(tok.kind, super::input::TokKind::Comment(..));
        let from = first
            .before
            .iter()
            .filter(comment)
            .map(|tok| tok.position)
            .fold(first.tok.position, usize::min);
        let to = last
            .after
            .iter()
            .filter(comment)
            .map(|tok| tok.position + tok.text.len())
            .fold(last.tok.position + last.tok.text.len(), usize::max);
        Ok(self.source[from..to].encode_utf16().count())
    }

    pub(super) fn sync(&mut self, node: &Node) -> Result<(), Unformatted> {
        let synced = matches!(
            node.kind,
            NodeKind::ImportDecl
                | NodeKind::ClassDecl
                | NodeKind::InterfaceDecl
                | NodeKind::EnumDecl
                | NodeKind::RecordDecl
                | NodeKind::AnnotationTypeDecl
                | NodeKind::Annotation
                | NodeKind::ElementValuePair
                | NodeKind::TypeParameter
                | NodeKind::FieldDecl
                | NodeKind::LocalVariableDecl
                | NodeKind::FormalParameter
                | NodeKind::RecordComponent
                | NodeKind::MethodDecl
                | NodeKind::ConstructorDecl
                | NodeKind::CompactConstructorDecl
                | NodeKind::PrimitiveType
                | NodeKind::VoidType
                | NodeKind::Initializer
                | NodeKind::ClassType
                | NodeKind::ArrayType
                | NodeKind::Wildcard
                | NodeKind::Block
                | NodeKind::ExpressionStmt
                | NodeKind::IfStmt
                | NodeKind::WhileStmt
                | NodeKind::DoStmt
                | NodeKind::ForStmt
                | NodeKind::ForEachStmt
                | NodeKind::TryStmt
                | NodeKind::CatchClause
                | NodeKind::SwitchStmt
                | NodeKind::SwitchRule
                | NodeKind::SwitchGroup
                | NodeKind::ReturnStmt
                | NodeKind::BreakStmt
                | NodeKind::ContinueStmt
                | NodeKind::ThrowStmt
                | NodeKind::YieldStmt
                | NodeKind::SynchronizedStmt
                | NodeKind::LabeledStmt
                | NodeKind::AssertStmt
                | NodeKind::EmptyStmt
                | NodeKind::Literal
                | NodeKind::Name
                | NodeKind::FieldAccess
                | NodeKind::MethodCall
                | NodeKind::NewClass
                | NodeKind::ArrayAccess
                | NodeKind::Unary
                | NodeKind::Postfix
                | NodeKind::Binary
                | NodeKind::InstanceOf
                | NodeKind::Conditional
                | NodeKind::Assignment
                | NodeKind::Lambda
                | NodeKind::Cast
                | NodeKind::ClassLiteral
                | NodeKind::This
                | NodeKind::Super
                | NodeKind::SwitchExpr
                | NodeKind::TypePattern
        );
        let mut pending: Vec<&Element> = node.children.iter().rev().collect();
        while let (true, Some(element)) = (synced, pending.pop()) {
            match element {
                Element::Token(token) => return self.builder.add(Op::Sync(token.start)),
                Element::Node(child) => pending.extend(child.children.iter().rev()),
            }
        }
        Ok(())
    }

    fn scan(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        self.sync(node)?;
        match node.kind {
            NodeKind::ClassDecl | NodeKind::InterfaceDecl => self.class_declaration(node),
            NodeKind::EnumDecl => self.enum_declaration(node),
            NodeKind::AnnotationTypeDecl => self.annotation_type(node),
            NodeKind::MethodDecl | NodeKind::ConstructorDecl | NodeKind::CompactConstructorDecl => {
                self.method(node)
            }
            NodeKind::RecordDecl => self.record_declaration(node),
            NodeKind::ModuleDecl => self.module_declaration(node),
            NodeKind::Initializer => self.initializer(node),
            NodeKind::LocalClassDecl => match node.children.as_slice() {
                [Element::Node(declaration)] => self.scan(declaration),
                _ => Err(Unformatted::Deferred),
            },
            NodeKind::Block => self.block(node, Collapse::Never, BlankLine::No, BlankLine::No),
            NodeKind::IfStmt => self.if_statement(node),
            NodeKind::WhileStmt => self.while_statement(node),
            NodeKind::ForStmt => self.for_statement(node),
            NodeKind::ForEachStmt => self.for_each_statement(node),
            NodeKind::DoStmt => self.do_statement(node),
            NodeKind::TryStmt => self.try_statement(node),
            NodeKind::Lambda => self.lambda(node),
            NodeKind::Cast => self.cast(node),
            NodeKind::InstanceOf => self.instance_of(node),
            NodeKind::Conditional => self.conditional(node),
            NodeKind::NewClass => self.new_class(node),
            NodeKind::NewArray => self.new_array(node),
            NodeKind::MethodReference => self.method_reference(node),
            NodeKind::SwitchStmt | NodeKind::SwitchExpr => self.switch_statement(node),
            NodeKind::TypePattern => self.type_pattern(node),
            NodeKind::RecordPattern => self.record_pattern(node),
            NodeKind::AssertStmt => self.assert_statement(node),
            NodeKind::ThrowStmt | NodeKind::YieldStmt => self.throw_statement(node),
            NodeKind::SynchronizedStmt => self.synchronized_statement(node),
            NodeKind::LabeledStmt => self.labeled_statement(node),
            NodeKind::BreakStmt | NodeKind::ContinueStmt => self.jump_statement(node),
            NodeKind::EmptyStmt => match node.children.as_slice() {
                [Element::Token(semicolon)] => {
                    self.builder.add(FORCED)?;
                    self.token(semicolon, ZERO, None)
                }
                _ => Err(Unformatted::Deferred),
            },
            NodeKind::ReturnStmt => self.return_statement(node),
            NodeKind::ExpressionStmt => self.expression_statement(node),
            NodeKind::Annotation => self.annotation(node),
            NodeKind::ArrayInitializer | NodeKind::ElementValueArray => {
                self.array_initializer(node, None)
            }
            NodeKind::Literal
            | NodeKind::Name
            | NodeKind::PrimitiveType
            | NodeKind::VoidType
            | NodeKind::MatchAllPattern => {
                let [Element::Token(token)] = node.children.as_slice() else {
                    return Err(Unformatted::Deferred);
                };
                self.token(token, ZERO, None)
            }
            NodeKind::This | NodeKind::Super => match node.children.as_slice() {
                [Element::Token(token)] => self.token(token, ZERO, None),
                _ => self.dot(node),
            },
            NodeKind::FieldAccess
            | NodeKind::MethodCall
            | NodeKind::ArrayAccess
            | NodeKind::ClassLiteral => self.dot(node),
            NodeKind::ClassType => {
                let segments = self.segments(&node.children)?;
                self.parameterized(&segments)
            }
            NodeKind::ArrayType => {
                let [Element::Node(base), dims @ ..] = node.children.as_slice() else {
                    return Err(Unformatted::Deferred);
                };
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.scan(base)?;
                self.dimensions(dims)?;
                self.builder.add(CLOSE)
            }
            NodeKind::Wildcard => match node.children.as_slice() {
                [Element::Token(question)] => {
                    self.builder.add(OPEN_ZERO)?;
                    self.token(question, ZERO, None)?;
                    self.builder.add(CLOSE)
                }
                [
                    Element::Token(question),
                    Element::Token(keyword),
                    Element::Node(bound),
                ] => {
                    self.builder.add(OPEN_ZERO)?;
                    self.token(question, ZERO, None)?;
                    self.builder.add(OPEN_PLUS_FOUR)?;
                    self.builder.add(SPACE)?;
                    self.token(keyword, ZERO, None)?;
                    self.builder.add(SPACE_BREAK)?;
                    self.scan(bound)?;
                    self.builder.add(CLOSE)?;
                    self.builder.add(CLOSE)
                }
                _ => Err(Unformatted::Deferred),
            },
            NodeKind::Unary => {
                let [Element::Token(operator), Element::Node(operand)] = node.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                let text = &self.source[operator.start..operator.end];
                self.token(operator, ZERO, None)?;
                if matches!(text, "-" | "+") && operand.kind == NodeKind::Unary {
                    self.builder.add(SPACE)?;
                }
                self.scan(operand)
            }
            NodeKind::Postfix => {
                let [Element::Node(operand), Element::Token(operator)] = node.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                self.scan(operand)?;
                self.token(operator, ZERO, None)
            }
            NodeKind::Parenthesized => {
                let [
                    Element::Token(open),
                    Element::Node(expression),
                    Element::Token(close),
                ] = node.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                self.token(open, ZERO, None)?;
                self.scan(expression)?;
                self.token(close, ZERO, None)
            }
            NodeKind::Assignment => {
                let [Element::Node(variable), operator @ .., Element::Node(value)] =
                    node.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.scan(variable)?;
                self.builder.add(SPACE)?;
                for operator in operator {
                    let Element::Token(operator) = operator else {
                        return Err(Unformatted::Deferred);
                    };
                    self.token(operator, ZERO, None)?;
                }
                self.builder.add(SPACE_BREAK)?;
                self.scan(value)?;
                self.builder.add(CLOSE)
            }
            NodeKind::Binary => self.binary(node),
            NodeKind::CompilationUnit
            | NodeKind::PackageDecl
            | NodeKind::ImportDecl
            | NodeKind::ModuleDirective
            | NodeKind::QualifiedName
            | NodeKind::Modifiers
            | NodeKind::AnnotationArguments
            | NodeKind::ElementValuePair
            | NodeKind::TypeParameters
            | NodeKind::TypeParameter
            | NodeKind::TypeBound
            | NodeKind::ExtendsClause
            | NodeKind::ImplementsClause
            | NodeKind::PermitsClause
            | NodeKind::ClassBody
            | NodeKind::EnumBody
            | NodeKind::EnumConstant
            | NodeKind::RecordHeader
            | NodeKind::RecordComponent
            | NodeKind::FieldDecl
            | NodeKind::VariableDeclarator
            | NodeKind::FormalParameters
            | NodeKind::FormalParameter
            | NodeKind::ReceiverParameter
            | NodeKind::ThrowsClause
            | NodeKind::DefaultValue
            | NodeKind::EmptyDecl
            | NodeKind::Dimension
            | NodeKind::TypeArguments
            | NodeKind::LocalVariableDecl
            | NodeKind::ForInit
            | NodeKind::ForUpdate
            | NodeKind::ResourceSpecification
            | NodeKind::Resource
            | NodeKind::CatchClause
            | NodeKind::CatchType
            | NodeKind::FinallyClause
            | NodeKind::SwitchBlock
            | NodeKind::SwitchRule
            | NodeKind::SwitchGroup
            | NodeKind::SwitchLabel
            | NodeKind::Guard
            | NodeKind::Arguments
            | NodeKind::LambdaParameters => Err(Unformatted::Deferred),
        }
    }
}
