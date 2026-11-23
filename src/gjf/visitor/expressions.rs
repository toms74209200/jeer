use super::super::builder::{Op, Unformatted};
use super::super::doc::{BreakMode, Indent};
use super::super::output::BlankLine;
use super::declarations::{Declaration, field_annotation_direction};
use super::statements::Collapse;
use super::{
    BREAK, CLOSE, FILL, FILL_SPACE, FORCED, MINUS_TWO, OPEN_PLUS_FOUR, OPEN_PLUS_TWO, OPEN_ZERO,
    PLUS_FOUR, PLUS_TWO, SPACE, SPACE_BREAK, Visitor, ZERO,
};
use crate::lexer::token::{Punct, Token, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Literal {
    Int,
    Long,
    Float,
    Double,
    Boolean,
    Char,
    String,
    Null,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpressionKind {
    Literal(Literal),
    Identifier,
    MemberSelect,
    NewArray,
    Operator(NodeKind, crate::lexer::token::Punct),
    Other(NodeKind),
}

#[derive(Clone, Copy)]
enum Items {
    Short,
    Long,
}

pub(super) enum Table {
    Rows(usize),
    No,
}

struct Call<'n> {
    type_arguments: Option<&'n Node>,
    arguments: &'n Node,
}

struct Index<'n> {
    open: &'n Token,
    index: &'n Node,
    close: &'n Token,
}

struct Link<'n> {
    dot: Option<&'n Token>,
    name: &'n Token,
    call: Option<Call<'n>>,
    indices: Vec<Index<'n>>,
    end: usize,
}

pub(super) type Segment<'n> = (Option<&'n Token>, &'n Token, Option<&'n Node>);

enum Head<'n> {
    Node(&'n Node, Vec<Index<'n>>),
    Type(Vec<Segment<'n>>),
}

impl<'a> Visitor<'a> {
    pub(super) fn qualified_name(&mut self, name: &'a Node) -> Result<(), Unformatted> {
        let mut links = Vec::new();
        let mut dot = None;
        for part in &name.children {
            let Element::Token(part) = part else {
                return Err(Unformatted::Deferred);
            };
            match part.kind {
                TokenKind::Punct(_) => dot = Some(part),
                TokenKind::Identifier | TokenKind::Keyword(_) => links.push(Link {
                    dot: dot.take(),
                    name: part,
                    call: None,
                    indices: Vec::new(),
                    end: part.end,
                }),
                TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::JavadocComment
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::IntegerLiteral
                | TokenKind::FloatingLiteral
                | TokenKind::CharLiteral
                | TokenKind::StringLiteral
                | TokenKind::TextBlock
                | TokenKind::Eof => return Err(Unformatted::Deferred),
            }
        }
        match links.as_slice() {
            [single] => self.token(single.name, ZERO, None),
            _ => {
                let (start, _) = self.span(name)?;
                self.chain(None, links, start)
            }
        }
    }

    pub(super) fn segments(&self, node: &'a Node) -> Result<Vec<Segment<'a>>, Unformatted> {
        let mut segments: Vec<Segment<'a>> = Vec::new();
        let mut dot = None;
        for child in &node.children {
            match child {
                Element::Token(token) if matches!(token.kind, TokenKind::Punct(_)) => {
                    dot = Some(token);
                }
                Element::Token(token) => segments.push((dot.take(), token, None)),
                Element::Node(arguments) if arguments.kind == NodeKind::TypeArguments => {
                    let last = segments.last_mut().ok_or(Unformatted::Deferred)?;
                    last.2 = Some(arguments);
                }
                Element::Node(_) => return Err(Unformatted::Deferred),
            }
        }
        Ok(segments)
    }

    pub(super) fn parameterized(&mut self, segments: &[Segment<'a>]) -> Result<(), Unformatted> {
        let Some(&(_, _, arguments)) = segments.last() else {
            return Err(Unformatted::Deferred);
        };
        let Some(arguments) = arguments else {
            return self.selected(segments);
        };
        let [Element::Token(open), inner @ .., Element::Token(close)] =
            arguments.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        match inner.is_empty() {
            true => {
                self.selected(segments)?;
                self.token(open, ZERO, None)?;
                self.token(close, ZERO, None)
            }
            false => {
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.selected(segments)?;
                self.token(open, ZERO, None)?;
                self.builder.add(BREAK)?;
                self.builder.add(OPEN_ZERO)?;
                for child in inner {
                    match child {
                        Element::Token(comma) => {
                            self.token(comma, ZERO, None)?;
                            self.builder.add(SPACE_BREAK)?;
                        }
                        Element::Node(argument) => self.scan(argument)?,
                    }
                }
                self.builder.add(CLOSE)?;
                self.builder.add(CLOSE)?;
                self.token(close, ZERO, None)
            }
        }
    }

    fn selected(&mut self, segments: &[Segment<'a>]) -> Result<(), Unformatted> {
        let [(_, first, _), ..] = segments else {
            return Err(Unformatted::Deferred);
        };
        if segments.len() == 1 {
            return self.token(first, ZERO, None);
        }
        let last = segments.len() - 1;
        let split = segments[..last]
            .iter()
            .rposition(|segment| segment.2.is_some());
        let (head, from) = match split {
            Some(index) => (Some(Head::Type(segments[..=index].to_vec())), index + 1),
            None => (None, 0),
        };
        let links = segments[from..]
            .iter()
            .map(|&(dot, name, _)| Link {
                dot,
                name,
                call: None,
                indices: Vec::new(),
                end: name.end,
            })
            .collect();
        self.chain(head, links, first.start)
    }

    pub(super) fn dot(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let (start, _) = self.span(node)?;
        let mut links = Vec::new();
        let mut current = node;
        let head = loop {
            let (_, end) = self.span(current)?;
            let mut indices = Vec::new();
            while current.kind == NodeKind::ArrayAccess {
                let [
                    Element::Node(base),
                    Element::Token(open),
                    Element::Node(index),
                    Element::Token(close),
                ] = current.children.as_slice()
                else {
                    return Err(Unformatted::Deferred);
                };
                indices.push(Index { open, index, close });
                current = base;
            }
            indices.reverse();
            match (current.kind, current.children.as_slice()) {
                (
                    NodeKind::FieldAccess
                    | NodeKind::This
                    | NodeKind::Super
                    | NodeKind::ClassLiteral,
                    [
                        Element::Node(receiver),
                        Element::Token(dot),
                        Element::Token(name),
                    ],
                ) => {
                    links.push(Link {
                        dot: Some(dot),
                        name,
                        call: None,
                        indices,
                        end,
                    });
                    current = receiver;
                }
                (
                    NodeKind::MethodCall,
                    [
                        Element::Node(receiver),
                        Element::Token(dot),
                        Element::Node(type_arguments),
                        Element::Token(name),
                        Element::Node(arguments),
                    ],
                ) => {
                    links.push(Link {
                        dot: Some(dot),
                        name,
                        call: Some(Call {
                            type_arguments: Some(type_arguments),
                            arguments,
                        }),
                        indices,
                        end,
                    });
                    current = receiver;
                }
                (
                    NodeKind::MethodCall,
                    [
                        Element::Node(receiver),
                        Element::Token(dot),
                        Element::Token(name),
                        Element::Node(arguments),
                    ],
                ) => {
                    links.push(Link {
                        dot: Some(dot),
                        name,
                        call: Some(Call {
                            type_arguments: None,
                            arguments,
                        }),
                        indices,
                        end,
                    });
                    current = receiver;
                }
                (NodeKind::MethodCall, [Element::Node(callee), Element::Node(arguments)]) => {
                    let [Element::Token(name)] = callee.children.as_slice() else {
                        return Err(Unformatted::Deferred);
                    };
                    links.push(Link {
                        dot: None,
                        name,
                        call: Some(Call {
                            type_arguments: None,
                            arguments,
                        }),
                        indices,
                        end,
                    });
                    break None;
                }
                (
                    NodeKind::MethodCall,
                    [
                        Element::Node(type_arguments),
                        Element::Token(name),
                        Element::Node(arguments),
                    ],
                ) => {
                    links.push(Link {
                        dot: None,
                        name,
                        call: Some(Call {
                            type_arguments: Some(type_arguments),
                            arguments,
                        }),
                        indices,
                        end,
                    });
                    break None;
                }
                (NodeKind::Name | NodeKind::This | NodeKind::Super, [Element::Token(name)]) => {
                    links.push(Link {
                        dot: None,
                        name,
                        call: None,
                        indices,
                        end,
                    });
                    break None;
                }
                (NodeKind::ClassType, _) if indices.is_empty() => {
                    let segments = self.segments(current)?;
                    let split = segments.iter().rposition(|segment| segment.2.is_some());
                    let from = split.map_or(0, |index| index + 1);
                    for &(dot, name, _) in segments[from..].iter().rev() {
                        links.push(Link {
                            dot,
                            name,
                            call: None,
                            indices: Vec::new(),
                            end: name.end,
                        });
                    }
                    break split.map(|index| Head::Type(segments[..=index].to_vec()));
                }
                (
                    NodeKind::FieldAccess
                    | NodeKind::This
                    | NodeKind::Super
                    | NodeKind::ClassLiteral
                    | NodeKind::MethodCall
                    | NodeKind::Name
                    | NodeKind::ClassType,
                    _,
                )
                | (
                    NodeKind::CompilationUnit
                    | NodeKind::PackageDecl
                    | NodeKind::ImportDecl
                    | NodeKind::ModuleDecl
                    | NodeKind::ModuleDirective
                    | NodeKind::QualifiedName
                    | NodeKind::Modifiers
                    | NodeKind::Annotation
                    | NodeKind::AnnotationArguments
                    | NodeKind::ElementValuePair
                    | NodeKind::ElementValueArray
                    | NodeKind::ClassDecl
                    | NodeKind::InterfaceDecl
                    | NodeKind::EnumDecl
                    | NodeKind::RecordDecl
                    | NodeKind::AnnotationTypeDecl
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
                    | NodeKind::MethodDecl
                    | NodeKind::ConstructorDecl
                    | NodeKind::CompactConstructorDecl
                    | NodeKind::FormalParameters
                    | NodeKind::FormalParameter
                    | NodeKind::ReceiverParameter
                    | NodeKind::ThrowsClause
                    | NodeKind::DefaultValue
                    | NodeKind::Initializer
                    | NodeKind::EmptyDecl
                    | NodeKind::PrimitiveType
                    | NodeKind::ArrayType
                    | NodeKind::Dimension
                    | NodeKind::TypeArguments
                    | NodeKind::Wildcard
                    | NodeKind::VoidType
                    | NodeKind::Block
                    | NodeKind::LocalVariableDecl
                    | NodeKind::LocalClassDecl
                    | NodeKind::ExpressionStmt
                    | NodeKind::IfStmt
                    | NodeKind::WhileStmt
                    | NodeKind::DoStmt
                    | NodeKind::ForStmt
                    | NodeKind::ForInit
                    | NodeKind::ForUpdate
                    | NodeKind::ForEachStmt
                    | NodeKind::TryStmt
                    | NodeKind::ResourceSpecification
                    | NodeKind::Resource
                    | NodeKind::CatchClause
                    | NodeKind::CatchType
                    | NodeKind::FinallyClause
                    | NodeKind::SwitchStmt
                    | NodeKind::SwitchBlock
                    | NodeKind::SwitchRule
                    | NodeKind::SwitchGroup
                    | NodeKind::SwitchLabel
                    | NodeKind::Guard
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
                    | NodeKind::Arguments
                    | NodeKind::NewClass
                    | NodeKind::NewArray
                    | NodeKind::ArrayInitializer
                    | NodeKind::ArrayAccess
                    | NodeKind::Unary
                    | NodeKind::Postfix
                    | NodeKind::Binary
                    | NodeKind::InstanceOf
                    | NodeKind::Conditional
                    | NodeKind::Assignment
                    | NodeKind::Lambda
                    | NodeKind::LambdaParameters
                    | NodeKind::MethodReference
                    | NodeKind::Cast
                    | NodeKind::Parenthesized
                    | NodeKind::SwitchExpr
                    | NodeKind::TypePattern
                    | NodeKind::RecordPattern
                    | NodeKind::MatchAllPattern,
                    _,
                ) => break Some(Head::Node(current, indices)),
            }
        };
        links.reverse();
        let log_methods = [
            "at",
            "atConfig",
            "atDebug",
            "atFine",
            "atFiner",
            "atFinest",
            "atInfo",
            "atMostEvery",
            "atSevere",
            "atWarning",
            "every",
            "log",
            "logVarargs",
            "perUnique",
            "withCause",
            "withStackTrace",
        ];
        let name = |link: &Link| &self.source[link.name.start..link.name.end];
        let logging = match (head.is_none(), links.as_slice()) {
            (true, [receiver, calls @ .., last]) => {
                receiver.call.is_none()
                    && receiver.indices.is_empty()
                    && matches!(name(last), "log" | "logVarargs")
                    && calls.iter().chain([last]).all(|link| {
                        link.call.is_some()
                            && link.indices.is_empty()
                            && log_methods.contains(&name(link))
                    })
            }
            (_, _) => false,
        };
        match logging {
            true => self.prefixed_dot(&links, false, &[links.len() - 1], BreakMode::Fill),
            false => self.chain(head, links, start),
        }
    }

    fn chain(
        &mut self,
        head: Option<Head<'a>>,
        links: Vec<Link<'a>>,
        start: usize,
    ) -> Result<(), Unformatted> {
        let has_head = head.is_some();
        let mut need_dot = false;
        if let Some(head) = head {
            self.builder.add(OPEN_PLUS_FOUR)?;
            let indices = match head {
                Head::Node(node, indices) => {
                    self.scan(node)?;
                    indices
                }
                Head::Type(segments) => {
                    self.parameterized(&segments)?;
                    Vec::new()
                }
            };
            self.builder.add(BREAK)?;
            need_dot = true;
            self.indices(&indices)?;
            if links.is_empty() {
                return self.builder.add(CLOSE);
            }
        }
        let mut names = Vec::new();
        for link in &links {
            names.push(&self.source[link.name.start..link.name.end]);
            if link.call.is_some() || !link.indices.is_empty() {
                break;
            }
        }
        let mut prefixes: Vec<usize> = Vec::new();
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Case {
            Uppercase,
            Lowercase,
            UpperCamel,
            LowerCamel,
        }
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum State {
            Start,
            Type,
            FirstStaticMember,
            Reject,
            Ambiguous,
        }
        let mut state = State::Start;
        let mut type_length = None;
        for (index, name) in names.iter().enumerate() {
            let (mut first_upper, mut upper, mut lower, mut first) = (false, false, false, true);
            for c in name
                .encode_utf16()
                .filter_map(|unit| char::from_u32(u32::from(unit)))
                .filter(|c| c.is_alphabetic())
            {
                if first {
                    first_upper = c.is_uppercase();
                    first = false;
                }
                upper |= c.is_uppercase();
                lower |= c.is_lowercase();
            }
            let case = match (
                first_upper,
                lower || name.encode_utf16().count() == 1,
                upper,
            ) {
                (true, true, _) => Case::UpperCamel,
                (true, false, _) => Case::Uppercase,
                (false, _, true) => Case::LowerCamel,
                (false, _, false) => Case::Lowercase,
            };
            state = match (state, case) {
                (State::Start, Case::Uppercase) => State::Ambiguous,
                (State::Start, Case::LowerCamel) => State::Reject,
                (State::Start, Case::Lowercase) => State::Start,
                (State::Start | State::Type | State::Ambiguous, Case::UpperCamel) => State::Type,
                (State::Type, Case::Uppercase | Case::LowerCamel | Case::Lowercase) => {
                    State::FirstStaticMember
                }
                (State::Ambiguous, Case::Uppercase) => State::Ambiguous,
                (State::Ambiguous, Case::LowerCamel | Case::Lowercase)
                | (State::FirstStaticMember | State::Reject, _) => State::Reject,
            };
            match state {
                State::Reject => break,
                State::Type | State::FirstStaticMember => type_length = Some(index),
                State::Start | State::Ambiguous => {}
            }
        }
        if let Some(length) = type_length {
            prefixes.push(length);
        }
        let invocations: Vec<usize> = links
            .iter()
            .enumerate()
            .filter(|(_, link)| link.call.is_some() && link.indices.is_empty())
            .map(|(index, _)| index)
            .collect();
        let counted = invocations
            .iter()
            .filter(|&&index| index > 0 || has_head)
            .count();
        if let (1, Some(&first)) = (counted, invocations.first())
            && first > 0
            && !prefixes.contains(&first)
        {
            prefixes.push(first);
        }
        if let (true, Some(first)) = (prefixes.is_empty(), links.first())
            && first.dot.is_none()
            && first.call.is_none()
            && first.indices.is_empty()
            && matches!(
                &self.source[first.name.start..first.name.end],
                "this" | "super"
            )
        {
            prefixes.push(1);
        }
        let streams: Vec<usize> = invocations
            .iter()
            .copied()
            .filter(|&index| {
                let name = links[index].name;
                matches!(
                    &self.source[name.start..name.end],
                    "stream" | "parallelStream" | "toBuilder"
                )
            })
            .collect();
        for &stream in &streams {
            if !prefixes.contains(&stream) {
                prefixes.push(stream);
            }
        }
        match prefixes.is_empty() {
            true => self.regular_dot(&links, need_dot, start)?,
            false => {
                let fill = match streams.is_empty() {
                    true => BreakMode::Fill,
                    false => BreakMode::Unified,
                };
                self.prefixed_dot(&links, need_dot, &prefixes, fill)?;
            }
        }
        if has_head {
            self.builder.add(CLOSE)?;
        }
        Ok(())
    }

    fn regular_dot(
        &mut self,
        links: &[Link<'a>],
        need_dot: bool,
        start: usize,
    ) -> Result<(), Unformatted> {
        let trailing = links.len() > 1;
        if !need_dot {
            self.builder.add(OPEN_PLUS_FOUR)?;
        }
        let minimum = 4;
        let mut length = match need_dot {
            true => minimum,
            false => 0,
        };
        let mut dotted = need_dot;
        for (index, link) in links.iter().enumerate() {
            let broken = need_dot || (dotted && link.dot.is_some() && length > minimum);
            if let (true, Some(dot)) = (dotted, link.dot) {
                if length > minimum {
                    self.builder.add(BREAK)?;
                }
                self.token(dot, ZERO, None)?;
                length += 1;
            }
            let first_line = !broken && index + 1 < links.len();
            if let (true, Some(call)) = (first_line, &link.call) {
                self.do_not_hug(call.arguments);
            }
            let filled = match (
                &link.call,
                link.dot,
                links.len() >= 2,
                link.indices.is_empty(),
            ) {
                (
                    Some(Call {
                        type_arguments: None,
                        arguments,
                    }),
                    None,
                    true,
                    true,
                ) if self.source[link.name.start..link.name.end]
                    .encode_utf16()
                    .count()
                    <= 4 =>
                {
                    match arguments.children.as_slice() {
                        [
                            Element::Token(open),
                            Element::Node(argument),
                            Element::Token(close),
                        ] if argument.kind != NodeKind::Lambda => Some((open, argument, close)),
                        _ => None,
                    }
                }
                _ => None,
            };
            match filled {
                Some((open, argument, close)) => {
                    self.builder.add(OPEN_ZERO)?;
                    self.builder.add(OPEN_ZERO)?;
                    self.token(link.name, ZERO, None)?;
                    self.token(open, ZERO, None)?;
                    self.scan(argument)?;
                    self.builder.add(CLOSE)?;
                    self.token(close, ZERO, None)?;
                    self.builder.add(CLOSE)?;
                }
                None => {
                    let type_tag = self.builder.tag();
                    self.up_to_arguments(link, type_tag)?;
                    let indent = match trailing || dotted {
                        true => PLUS_FOUR,
                        false => ZERO,
                    };
                    let hugged = match first_line {
                        true => PLUS_FOUR,
                        false => ZERO,
                    };
                    self.arguments_and_indices(
                        link,
                        Indent::If(type_tag, Box::new(PLUS_FOUR), Box::new(ZERO)),
                        indent,
                        hugged,
                    )?;
                }
            }
            length += self.source[start..link.end].encode_utf16().count();
            dotted = true;
        }
        if !need_dot {
            self.builder.add(CLOSE)?;
        }
        Ok(())
    }

    fn prefixed_dot(
        &mut self,
        links: &[Link<'a>],
        need_dot: bool,
        prefixes: &[usize],
        fill: BreakMode,
    ) -> Result<(), Unformatted> {
        let trailing = prefixes.last().is_some_and(|&last| last + 1 < links.len());
        self.builder.add(OPEN_PLUS_FOUR)?;
        for _ in prefixes {
            self.builder.add(OPEN_ZERO)?;
        }
        let mut unconsumed = prefixes.to_vec();
        unconsumed.sort_unstable();
        unconsumed.reverse();
        let name_tag = self.builder.tag();
        let mut dotted = need_dot;
        for (index, link) in links.iter().enumerate() {
            if let (false, true, Some(call)) = (
                need_dot,
                (index == 0 || prefixes.contains(&index)) && index + 1 < links.len(),
                &link.call,
            ) {
                self.do_not_hug(call.arguments);
            }
            if let (true, Some(dot)) = (dotted, link.dot) {
                let mode = match unconsumed.last() {
                    Some(&prefix) if index <= prefix => fill,
                    Some(_) | None => BreakMode::Unified,
                };
                self.builder.add(Op::Break {
                    mode,
                    flat: "",
                    plus_indent: ZERO,
                    tag: Some(name_tag),
                })?;
                self.token(dot, ZERO, None)?;
            }
            let type_tag = self.builder.tag();
            self.up_to_arguments(link, type_tag)?;
            if unconsumed.last() == Some(&index) {
                self.builder.add(CLOSE)?;
                unconsumed.pop();
            }
            let otherwise = match trailing {
                true => PLUS_FOUR,
                false => ZERO,
            };
            self.arguments_and_indices(
                link,
                Indent::If(type_tag, Box::new(PLUS_FOUR), Box::new(ZERO)),
                Indent::If(name_tag, Box::new(PLUS_FOUR), Box::new(otherwise.clone())),
                Indent::If(name_tag, Box::new(ZERO), Box::new(otherwise)),
            )?;
            dotted = true;
        }
        self.builder.add(CLOSE)
    }

    fn up_to_arguments(
        &mut self,
        link: &Link<'a>,
        type_tag: super::super::doc::BreakTag,
    ) -> Result<(), Unformatted> {
        if let Some(Call {
            type_arguments: Some(type_arguments),
            ..
        }) = &link.call
        {
            self.builder.add(OPEN_PLUS_FOUR)?;
            self.type_argument_list(type_arguments, OPEN_ZERO)?;
            self.builder.add(Op::Break {
                mode: BreakMode::Unified,
                flat: "",
                plus_indent: ZERO,
                tag: Some(type_tag),
            })?;
            self.builder.add(CLOSE)?;
        }
        self.token(link.name, ZERO, None)
    }

    fn arguments_and_indices(
        &mut self,
        link: &Link<'a>,
        type_indent: Indent,
        indent: Indent,
        hugged_indent: Indent,
    ) -> Result<(), Unformatted> {
        if let Some(call) = &link.call {
            self.builder.add(Op::Open(type_indent))?;
            self.arguments(call.arguments, indent, hugged_indent)?;
            self.builder.add(CLOSE)?;
        }
        self.indices(&link.indices)
    }

    fn indices(&mut self, indices: &[Index<'a>]) -> Result<(), Unformatted> {
        if indices.is_empty() {
            return Ok(());
        }
        self.builder.add(OPEN_ZERO)?;
        for index in indices {
            self.token(index.open, ZERO, None)?;
            self.builder.add(FILL)?;
            self.scan(index.index)?;
            self.token(index.close, ZERO, None)?;
        }
        self.builder.add(CLOSE)
    }

    pub(super) fn do_not_hug(&mut self, arguments: &Node) {
        match arguments.children.as_slice() {
            [Element::Token(_), Element::Node(only), Element::Token(_)]
                if only.kind == NodeKind::Lambda
                    && matches!(
                        only.children.last(),
                        Some(Element::Node(Node {
                            kind: NodeKind::Block,
                            ..
                        }))
                    ) =>
            {
                self.not_huggable.insert(std::ptr::from_ref(only).addr());
            }
            _ => {}
        }
    }

    pub(super) fn arguments(
        &mut self,
        arguments: &'a Node,
        plus_indent: Indent,
        hugged_plus_indent: Indent,
    ) -> Result<(), Unformatted> {
        let [Element::Token(open), inner @ .., Element::Token(close)] =
            arguments.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let mut expressions = Vec::new();
        let mut commas = Vec::new();
        for child in inner {
            match child {
                Element::Node(expression) => expressions.push(expression),
                Element::Token(comma) => commas.push(comma),
            }
        }
        let tabular =
            expressions.len() % 2 == 0 && matches!(self.tabular(&expressions)?, Table::Rows(2));
        let mut literal = true;
        let mut format = false;
        let mut pending: Vec<&Node> = expressions.first().copied().into_iter().collect();
        while let Some(node) = pending.pop() {
            match (node.kind, node.children.as_slice()) {
                (NodeKind::Literal, [Element::Token(token)])
                    if token.kind == TokenKind::StringLiteral =>
                {
                    let decoded: Vec<char> = self
                        .decoded
                        .decoded(token.start, token.end)
                        .chars()
                        .collect();
                    let value = crate::lexer::literal::string_value(&decoded);
                    let chars: Vec<char> = value.chars().collect();
                    format |= value.contains('%')
                        || chars.windows(3).any(|window| {
                            window[0] == '{' && window[1].is_ascii_digit() && window[2] == '}'
                        });
                }
                (
                    NodeKind::Binary,
                    [
                        Element::Node(left),
                        Element::Token(operator),
                        Element::Node(right),
                    ],
                ) if &self.source[operator.start..operator.end] == "+" => {
                    pending.push(right);
                    pending.push(left);
                }
                (NodeKind::Literal | NodeKind::Binary, _)
                | (
                    NodeKind::CompilationUnit
                    | NodeKind::PackageDecl
                    | NodeKind::ImportDecl
                    | NodeKind::ModuleDecl
                    | NodeKind::ModuleDirective
                    | NodeKind::QualifiedName
                    | NodeKind::Modifiers
                    | NodeKind::Annotation
                    | NodeKind::AnnotationArguments
                    | NodeKind::ElementValuePair
                    | NodeKind::ElementValueArray
                    | NodeKind::ClassDecl
                    | NodeKind::InterfaceDecl
                    | NodeKind::EnumDecl
                    | NodeKind::RecordDecl
                    | NodeKind::AnnotationTypeDecl
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
                    | NodeKind::MethodDecl
                    | NodeKind::ConstructorDecl
                    | NodeKind::CompactConstructorDecl
                    | NodeKind::FormalParameters
                    | NodeKind::FormalParameter
                    | NodeKind::ReceiverParameter
                    | NodeKind::ThrowsClause
                    | NodeKind::DefaultValue
                    | NodeKind::Initializer
                    | NodeKind::EmptyDecl
                    | NodeKind::PrimitiveType
                    | NodeKind::ClassType
                    | NodeKind::ArrayType
                    | NodeKind::Dimension
                    | NodeKind::TypeArguments
                    | NodeKind::Wildcard
                    | NodeKind::VoidType
                    | NodeKind::Block
                    | NodeKind::LocalVariableDecl
                    | NodeKind::LocalClassDecl
                    | NodeKind::ExpressionStmt
                    | NodeKind::IfStmt
                    | NodeKind::WhileStmt
                    | NodeKind::DoStmt
                    | NodeKind::ForStmt
                    | NodeKind::ForInit
                    | NodeKind::ForUpdate
                    | NodeKind::ForEachStmt
                    | NodeKind::TryStmt
                    | NodeKind::ResourceSpecification
                    | NodeKind::Resource
                    | NodeKind::CatchClause
                    | NodeKind::CatchType
                    | NodeKind::FinallyClause
                    | NodeKind::SwitchStmt
                    | NodeKind::SwitchBlock
                    | NodeKind::SwitchRule
                    | NodeKind::SwitchGroup
                    | NodeKind::SwitchLabel
                    | NodeKind::Guard
                    | NodeKind::ReturnStmt
                    | NodeKind::BreakStmt
                    | NodeKind::ContinueStmt
                    | NodeKind::ThrowStmt
                    | NodeKind::YieldStmt
                    | NodeKind::SynchronizedStmt
                    | NodeKind::LabeledStmt
                    | NodeKind::AssertStmt
                    | NodeKind::EmptyStmt
                    | NodeKind::Name
                    | NodeKind::FieldAccess
                    | NodeKind::MethodCall
                    | NodeKind::Arguments
                    | NodeKind::NewClass
                    | NodeKind::NewArray
                    | NodeKind::ArrayInitializer
                    | NodeKind::ArrayAccess
                    | NodeKind::Unary
                    | NodeKind::Postfix
                    | NodeKind::InstanceOf
                    | NodeKind::Conditional
                    | NodeKind::Assignment
                    | NodeKind::Lambda
                    | NodeKind::LambdaParameters
                    | NodeKind::MethodReference
                    | NodeKind::Cast
                    | NodeKind::Parenthesized
                    | NodeKind::ClassLiteral
                    | NodeKind::This
                    | NodeKind::Super
                    | NodeKind::SwitchExpr
                    | NodeKind::TypePattern
                    | NodeKind::RecordPattern
                    | NodeKind::MatchAllPattern,
                    _,
                ) => literal = false,
            }
        }
        let huggable = match expressions.as_slice() {
            [only]
                if only.kind == NodeKind::Lambda
                    && !self
                        .not_huggable
                        .contains(&std::ptr::from_ref(*only).addr()) =>
            {
                Some(*only)
            }
            _ => None,
        };
        let opener = match huggable {
            Some(only) => {
                let point = self.builder.hug_point();
                self.hug_points
                    .insert(std::ptr::from_ref(only).addr(), point);
                let block = matches!(
                    only.children.last(),
                    Some(Element::Node(Node {
                        kind: NodeKind::Block,
                        ..
                    }))
                );
                self.builder.add(Op::OpenHuggable {
                    plus_indent,
                    hugged_plus_indent: match block {
                        true => ZERO,
                        false => hugged_plus_indent,
                    },
                    point,
                })?;
                Op::OpenHuggedArguments
            }
            None => {
                self.builder.add(Op::Open(plus_indent))?;
                OPEN_ZERO
            }
        };
        self.token(open, ZERO, None)?;
        match (
            expressions.as_slice(),
            tabular,
            expressions.len() >= 2 && literal && format,
        ) {
            ([], _, _) => {}
            (_, true, _) => {
                self.builder.add(FORCED)?;
                self.builder.add(OPEN_ZERO)?;
                for pair in (0..expressions.len()).step_by(2) {
                    if pair > 0 {
                        self.token(
                            commas.get(pair - 1).ok_or(Unformatted::Deferred)?,
                            ZERO,
                            None,
                        )?;
                        self.builder.add(FORCED)?;
                    }
                    self.builder.add(OPEN_PLUS_FOUR)?;
                    self.scan(expressions[pair])?;
                    self.token(commas.get(pair).ok_or(Unformatted::Deferred)?, ZERO, None)?;
                    self.builder.add(SPACE_BREAK)?;
                    self.scan(expressions.get(pair + 1).ok_or(Unformatted::Deferred)?)?;
                    self.builder.add(CLOSE)?;
                }
                self.builder.add(CLOSE)?;
            }
            ([first, rest @ ..], false, true) => {
                self.builder.add(BREAK)?;
                self.builder.add(OPEN_ZERO)?;
                self.scan(first)?;
                self.token(commas.first().ok_or(Unformatted::Deferred)?, ZERO, None)?;
                self.builder.add(SPACE_BREAK)?;
                self.builder.add(OPEN_ZERO)?;
                self.argument_list(rest, &commas[1..], OPEN_ZERO)?;
                self.builder.add(CLOSE)?;
                self.builder.add(CLOSE)?;
            }
            (all, false, false) => {
                self.builder.add(BREAK)?;
                self.argument_list(all, &commas, opener)?;
            }
        }
        self.token(close, ZERO, None)?;
        self.builder.add(CLOSE)
    }

    pub(super) fn lambda(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [
            Element::Node(parameters),
            Element::Token(arrow),
            Element::Node(body),
        ] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let parens = matches!(
            parameters.children.first(),
            Some(Element::Token(open)) if open.kind == TokenKind::Punct(Punct::LParen)
        );
        match parens {
            true => self.builder.add(OPEN_PLUS_FOUR)?,
            false => self.builder.add(OPEN_ZERO)?,
        }
        for child in &parameters.children {
            match child {
                Element::Token(comma) if comma.kind == TokenKind::Punct(Punct::Comma) => {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(SPACE_BREAK)?;
                }
                Element::Token(token) => self.token(token, ZERO, None)?,
                Element::Node(parameter) => {
                    let (modifiers, ty, varargs, name, dims) = match parameter.children.as_slice() {
                        [
                            Element::Node(modifiers),
                            Element::Node(ty),
                            Element::Token(varargs),
                            Element::Token(name),
                            dims @ ..,
                        ] if varargs.kind == TokenKind::Punct(Punct::Ellipsis) => {
                            (modifiers, ty, Some(varargs), name, dims)
                        }
                        [
                            Element::Node(modifiers),
                            Element::Node(ty),
                            Element::Token(name),
                            dims @ ..,
                        ] => (modifiers, ty, None, name, dims),
                        _ => return Err(Unformatted::Deferred),
                    };
                    self.sync(parameter)?;
                    self.declare_one(
                        Declaration::Local,
                        field_annotation_direction(modifiers),
                        Some(modifiers),
                        ty,
                        varargs,
                        name,
                        dims,
                        None,
                        None,
                    )?;
                }
            }
        }
        self.builder.add(CLOSE)?;
        self.builder.add(SPACE)?;
        self.token(arrow, ZERO, None)?;
        if let Some(point) = self.hug_points.remove(&std::ptr::from_ref(node).addr()) {
            self.builder.add(Op::HugPoint(point))?;
        }
        match body.kind == NodeKind::Block {
            true => {
                self.builder.add(OPEN_ZERO)?;
                self.builder.add(SPACE)?;
                self.block(body, Collapse::Empty, BlankLine::No, BlankLine::No)?;
            }
            false => {
                self.builder.add(OPEN_PLUS_FOUR)?;
                self.builder.add(SPACE_BREAK)?;
                self.scan(body)?;
            }
        }
        self.builder.add(CLOSE)
    }

    fn type_argument_list(
        &mut self,
        type_arguments: &'a Node,
        opener: Op<'static>,
    ) -> Result<(), Unformatted> {
        let [Element::Token(open), items @ .., Element::Token(close)] =
            type_arguments.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        self.token(open, ZERO, None)?;
        self.builder.add(opener)?;
        self.builder.add(Op::Fence)?;
        for item in items {
            match item {
                Element::Token(comma) => {
                    self.token(comma, ZERO, None)?;
                    self.builder.add(FILL_SPACE)?;
                }
                Element::Node(argument) => self.scan(argument)?,
            }
        }
        self.builder.add(CLOSE)?;
        self.token(close, ZERO, None)
    }

    pub(super) fn method_reference(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let [Element::Node(base), Element::Token(colons), rest @ ..] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let (type_arguments, name) = match rest {
            [Element::Token(name)] => (None, name),
            [Element::Node(type_arguments), Element::Token(name)] => (Some(type_arguments), name),
            _ => return Err(Unformatted::Deferred),
        };
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.scan(base)?;
        self.builder.add(BREAK)?;
        self.token(colons, ZERO, None)?;
        if let Some(type_arguments) = type_arguments {
            self.type_argument_list(type_arguments, OPEN_PLUS_FOUR)?;
        }
        self.token(name, ZERO, None)?;
        self.builder.add(CLOSE)
    }

    fn argument_list(
        &mut self,
        expressions: &[&'a Node],
        commas: &[&'a Token],
        opener: Op<'static>,
    ) -> Result<(), Unformatted> {
        self.builder.add(opener)?;
        let mode = match self.short_items(expressions)? {
            Items::Short => BreakMode::Fill,
            Items::Long => BreakMode::Unified,
        };
        for (index, expression) in expressions.iter().enumerate() {
            if index > 0 {
                self.token(
                    commas.get(index - 1).ok_or(Unformatted::Deferred)?,
                    ZERO,
                    None,
                )?;
                self.builder.add(Op::Break {
                    mode,
                    flat: " ",
                    plus_indent: ZERO,
                    tag: None,
                })?;
            }
            self.scan(expression)?;
        }
        self.builder.add(CLOSE)
    }

    fn short_items(&self, expressions: &[&'a Node]) -> Result<Items, Unformatted> {
        for expression in expressions {
            if self.actual_size(expression)? >= 10 {
                return Ok(Items::Long);
            }
        }
        Ok(Items::Short)
    }

    pub(super) fn binary(&mut self, node: &'a Node) -> Result<(), Unformatted> {
        let precedence = |visitor: &Self, node: &Node| match node.children.as_slice() {
            [Element::Node(_), operator @ .., Element::Node(_)]
                if node.kind == NodeKind::Binary =>
            {
                let text: String = operator
                    .iter()
                    .filter_map(|element| match element {
                        Element::Token(token) => {
                            Some(visitor.decoded.decoded(token.start, token.end))
                        }
                        Element::Node(_) => None,
                    })
                    .collect();
                match text.as_str() {
                    "||" => Some(4),
                    "&&" => Some(5),
                    "|" => Some(6),
                    "^" => Some(7),
                    "&" => Some(8),
                    "==" | "!=" => Some(9),
                    "<" | ">" | "<=" | ">=" => Some(10),
                    "<<" | ">>" | ">>>" => Some(11),
                    "+" | "-" => Some(12),
                    "*" | "/" | "%" => Some(13),
                    _ => None,
                }
            }
            _ => None,
        };
        let level = precedence(self, node).ok_or(Unformatted::Deferred)?;
        let mut operands: Vec<&Node> = Vec::new();
        let mut operators: Vec<&[Element]> = Vec::new();
        let mut pending: Vec<Result<&Node, &[Element]>> = vec![Ok(node)];
        while let Some(work) = pending.pop() {
            match work {
                Err(operator) => operators.push(operator),
                Ok(current) if precedence(self, current) == Some(level) => {
                    let [Element::Node(left), operator @ .., Element::Node(right)] =
                        current.children.as_slice()
                    else {
                        return Err(Unformatted::Deferred);
                    };
                    pending.push(Ok(right));
                    pending.push(Err(operator));
                    pending.push(Ok(left));
                }
                Ok(operand) => operands.push(operand),
            }
        }
        let mode = match self.short_items(&operands)? {
            Items::Short => BreakMode::Fill,
            Items::Long => BreakMode::Unified,
        };
        self.builder.add(OPEN_PLUS_FOUR)?;
        self.scan(operands.first().ok_or(Unformatted::Deferred)?)?;
        for (operator, operand) in operators.iter().zip(&operands[1..]) {
            self.builder.add(Op::Break {
                mode,
                flat: " ",
                plus_indent: ZERO,
                tag: None,
            })?;
            for token in operator.iter() {
                let Element::Token(token) = token else {
                    return Err(Unformatted::Deferred);
                };
                self.token(token, ZERO, None)?;
            }
            self.builder.add(SPACE)?;
            self.scan(operand)?;
        }
        self.builder.add(CLOSE)
    }

    pub(super) fn array_initializer(
        &mut self,
        node: &'a Node,
        annotation_distance: Option<usize>,
    ) -> Result<(), Unformatted> {
        let [Element::Token(open), inner @ .., Element::Token(close)] = node.children.as_slice()
        else {
            return Err(Unformatted::Deferred);
        };
        let mut expressions = Vec::new();
        let mut commas = Vec::new();
        for child in inner {
            match child {
                Element::Node(expression) => expressions.push(expression),
                Element::Token(comma) => commas.push(comma),
            }
        }
        let trailing_comma = matches!(inner.last(), Some(Element::Token(_)));
        let nested = annotation_distance.map(|distance| distance + 1);
        if expressions.is_empty() {
            self.token(open, ZERO, Some(PLUS_TWO))?;
            for comma in &commas {
                self.token(comma, ZERO, None)?;
            }
            return self.token(close, PLUS_TWO, None);
        }
        match self.tabular(&expressions)? {
            Table::Rows(columns) => {
                self.builder.add(OPEN_PLUS_TWO)?;
                self.token(open, ZERO, None)?;
                self.builder.add(FORCED)?;
                for (row, items) in expressions.chunks(columns).enumerate() {
                    if row > 0 {
                        self.builder.add(FORCED)?;
                    }
                    let array = matches!(
                        items[0].kind,
                        NodeKind::ArrayInitializer | NodeKind::ElementValueArray
                    );
                    match array || columns == 1 {
                        true => self.builder.add(OPEN_ZERO)?,
                        false => self.builder.add(OPEN_PLUS_FOUR)?,
                    }
                    for (column, item) in items.iter().enumerate() {
                        let position = row * columns + column;
                        if column > 0 {
                            self.token(
                                commas.get(position - 1).ok_or(Unformatted::Deferred)?,
                                ZERO,
                                None,
                            )?;
                            self.builder.add(FILL_SPACE)?;
                        }
                        self.element(item, nested)?;
                    }
                    if let Some(comma) = commas.get(row * columns + items.len() - 1) {
                        self.token(comma, ZERO, None)?;
                    }
                    self.builder.add(CLOSE)?;
                }
                self.builder.add(Op::Break {
                    mode: BreakMode::Unified,
                    flat: "",
                    plus_indent: MINUS_TWO,
                    tag: None,
                })?;
                self.builder.add(CLOSE)?;
                self.token(close, PLUS_TWO, None)
            }
            Table::No => {
                let in_pair = annotation_distance == Some(1);
                let items = self.short_items(&expressions)?;
                let mode = match items {
                    Items::Short => BreakMode::Fill,
                    Items::Long => BreakMode::Unified,
                };
                let own_line = match (items, in_pair) {
                    (Items::Short, _) | (Items::Long, false) => true,
                    (Items::Long, true) => false,
                };
                self.builder.add(OPEN_PLUS_TWO)?;
                self.token(open, ZERO, Some(PLUS_TWO))?;
                let first = match trailing_comma {
                    true => BreakMode::Forced,
                    false => BreakMode::Unified,
                };
                self.builder.add(Op::Break {
                    mode: first,
                    flat: "",
                    plus_indent: ZERO,
                    tag: None,
                })?;
                if own_line {
                    self.builder.add(OPEN_ZERO)?;
                }
                for (index, expression) in expressions.iter().enumerate() {
                    if index > 0 {
                        self.token(
                            commas.get(index - 1).ok_or(Unformatted::Deferred)?,
                            ZERO,
                            None,
                        )?;
                        self.builder.add(Op::Break {
                            mode,
                            flat: " ",
                            plus_indent: ZERO,
                            tag: None,
                        })?;
                    }
                    self.element(expression, nested)?;
                }
                if let (true, Some(comma)) = (trailing_comma, commas.last()) {
                    self.token(comma, ZERO, None)?;
                }
                if own_line {
                    self.builder.add(CLOSE)?;
                }
                self.builder.add(Op::Break {
                    mode: BreakMode::Unified,
                    flat: "",
                    plus_indent: MINUS_TWO,
                    tag: None,
                })?;
                self.builder.add(CLOSE)?;
                self.token(close, PLUS_TWO, None)
            }
        }
    }

    pub(super) fn element(
        &mut self,
        element: &'a Node,
        annotation_distance: Option<usize>,
    ) -> Result<(), Unformatted> {
        match matches!(
            element.kind,
            NodeKind::ArrayInitializer | NodeKind::ElementValueArray
        ) {
            true => self.array_initializer(element, annotation_distance),
            false => self.scan(element),
        }
    }

    fn tabular(&self, expressions: &[&'a Node]) -> Result<Table, Unformatted> {
        if expressions.is_empty() {
            return Ok(Table::No);
        }
        let mut columns = Vec::new();
        for expression in expressions {
            let (start, _) = self.span(expression)?;
            let tokens = &self.builder.input.tokens;
            let index = tokens
                .partition_point(|token| token.tok.position <= start)
                .checked_sub(1)
                .ok_or(Unformatted::Deferred)?;
            let token = &tokens[index];
            let mut column = token.tok.column;
            let mut position = token.tok.position;
            for tok in &token.before {
                if self.source[tok.position..token.tok.position].contains('\n') {
                    break;
                }
                if matches!(tok.kind, super::super::input::TokKind::Comment(..))
                    && tok.position < position
                {
                    position = tok.position;
                    column = tok.column;
                }
            }
            columns.push(column);
        }
        let leaves = |row: &[&Node]| {
            let mut size = 0;
            let mut pending: Vec<&Node> = row.to_vec();
            while let Some(node) = pending.pop() {
                match node.kind == NodeKind::ArrayInitializer
                    || node.kind == NodeKind::ElementValueArray
                {
                    true => pending.extend(node.children.iter().filter_map(|child| match child {
                        Element::Node(node) => Some(node),
                        Element::Token(_) => None,
                    })),
                    false => size += 1,
                }
            }
            size
        };
        let start = columns[0];
        let mut rows: Vec<&[&Node]> = Vec::new();
        let mut index = 0;
        while index < expressions.len() {
            if columns[index] != start {
                return Ok(Table::No);
            }
            let from = index;
            index += 1;
            while index < expressions.len() && columns[index] > start {
                index += 1;
            }
            let row = &expressions[from..index];
            if rows.is_empty() && (index == expressions.len() || leaves(row) <= 1) {
                return Ok(Table::No);
            }
            rows.push(row);
        }
        let parallel = |column: usize, at_least: usize| {
            let mut counts: Vec<(ExpressionKind, usize)> = Vec::new();
            for row in &rows {
                let Some(item) = row.get(column) else {
                    continue;
                };
                let item = match (item.kind, item.children.as_slice()) {
                    (NodeKind::Unary, [_, Element::Node(operand)])
                    | (NodeKind::Postfix, [Element::Node(operand), _]) => operand,
                    (NodeKind::Unary | NodeKind::Postfix, _)
                    | (
                        NodeKind::CompilationUnit
                        | NodeKind::PackageDecl
                        | NodeKind::ImportDecl
                        | NodeKind::ModuleDecl
                        | NodeKind::ModuleDirective
                        | NodeKind::QualifiedName
                        | NodeKind::Modifiers
                        | NodeKind::Annotation
                        | NodeKind::AnnotationArguments
                        | NodeKind::ElementValuePair
                        | NodeKind::ElementValueArray
                        | NodeKind::ClassDecl
                        | NodeKind::InterfaceDecl
                        | NodeKind::EnumDecl
                        | NodeKind::RecordDecl
                        | NodeKind::AnnotationTypeDecl
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
                        | NodeKind::MethodDecl
                        | NodeKind::ConstructorDecl
                        | NodeKind::CompactConstructorDecl
                        | NodeKind::FormalParameters
                        | NodeKind::FormalParameter
                        | NodeKind::ReceiverParameter
                        | NodeKind::ThrowsClause
                        | NodeKind::DefaultValue
                        | NodeKind::Initializer
                        | NodeKind::EmptyDecl
                        | NodeKind::PrimitiveType
                        | NodeKind::ClassType
                        | NodeKind::ArrayType
                        | NodeKind::Dimension
                        | NodeKind::TypeArguments
                        | NodeKind::Wildcard
                        | NodeKind::VoidType
                        | NodeKind::Block
                        | NodeKind::LocalVariableDecl
                        | NodeKind::LocalClassDecl
                        | NodeKind::ExpressionStmt
                        | NodeKind::IfStmt
                        | NodeKind::WhileStmt
                        | NodeKind::DoStmt
                        | NodeKind::ForStmt
                        | NodeKind::ForInit
                        | NodeKind::ForUpdate
                        | NodeKind::ForEachStmt
                        | NodeKind::TryStmt
                        | NodeKind::ResourceSpecification
                        | NodeKind::Resource
                        | NodeKind::CatchClause
                        | NodeKind::CatchType
                        | NodeKind::FinallyClause
                        | NodeKind::SwitchStmt
                        | NodeKind::SwitchBlock
                        | NodeKind::SwitchRule
                        | NodeKind::SwitchGroup
                        | NodeKind::SwitchLabel
                        | NodeKind::Guard
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
                        | NodeKind::Arguments
                        | NodeKind::NewClass
                        | NodeKind::NewArray
                        | NodeKind::ArrayInitializer
                        | NodeKind::ArrayAccess
                        | NodeKind::Binary
                        | NodeKind::InstanceOf
                        | NodeKind::Conditional
                        | NodeKind::Assignment
                        | NodeKind::Lambda
                        | NodeKind::LambdaParameters
                        | NodeKind::MethodReference
                        | NodeKind::Cast
                        | NodeKind::Parenthesized
                        | NodeKind::ClassLiteral
                        | NodeKind::This
                        | NodeKind::Super
                        | NodeKind::SwitchExpr
                        | NodeKind::TypePattern
                        | NodeKind::RecordPattern
                        | NodeKind::MatchAllPattern,
                        _,
                    ) => item,
                };
                let key = self.expression_kind(item);
                match counts.iter_mut().find(|(existing, _)| *existing == key) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((key, 1)),
                }
            }
            counts.iter().any(|(_, count)| *count >= at_least)
        };
        let width = rows[0].len();
        if !parallel(0, rows.len()) {
            return Ok(Table::No);
        }
        for column in 1..width {
            if !parallel(column, rows.len() / 2 + 1) {
                return Ok(Table::No);
            }
        }
        if rows.len() == 2 {
            return match width == rows[1].len() {
                true => Ok(Table::Rows(width)),
                false => Ok(Table::No),
            };
        }
        if rows[1..rows.len() - 1].iter().any(|row| row.len() != width)
            || width < rows[rows.len() - 1].len()
        {
            return Ok(Table::No);
        }
        Ok(Table::Rows(width))
    }

    fn expression_kind(&self, node: &Node) -> ExpressionKind {
        let first_token = node.children.iter().find_map(|child| match child {
            Element::Token(token) => Some(token),
            Element::Node(_) => None,
        });
        let operator = |kind: NodeKind| match first_token.map(|token| token.kind) {
            Some(TokenKind::Punct(punct)) => ExpressionKind::Operator(kind, punct),
            Some(
                TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::JavadocComment
                | TokenKind::Identifier
                | TokenKind::Keyword(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::IntegerLiteral
                | TokenKind::FloatingLiteral
                | TokenKind::CharLiteral
                | TokenKind::StringLiteral
                | TokenKind::TextBlock
                | TokenKind::Eof,
            )
            | None => ExpressionKind::Other(kind),
        };
        match node.kind {
            NodeKind::Literal => {
                let Some(token) = first_token else {
                    return ExpressionKind::Other(NodeKind::Literal);
                };
                let last = self.source[token.start..token.end].chars().last();
                let literal = match (token.kind, last) {
                    (TokenKind::IntegerLiteral, Some('l' | 'L')) => Literal::Long,
                    (TokenKind::IntegerLiteral, _) => Literal::Int,
                    (TokenKind::FloatingLiteral, Some('f' | 'F')) => Literal::Float,
                    (TokenKind::FloatingLiteral, _) => Literal::Double,
                    (TokenKind::True | TokenKind::False, _) => Literal::Boolean,
                    (TokenKind::CharLiteral, _) => Literal::Char,
                    (TokenKind::StringLiteral | TokenKind::TextBlock, _) => Literal::String,
                    (
                        TokenKind::Null
                        | TokenKind::Whitespace
                        | TokenKind::Newline
                        | TokenKind::LineComment
                        | TokenKind::BlockComment
                        | TokenKind::JavadocComment
                        | TokenKind::Identifier
                        | TokenKind::Keyword(_)
                        | TokenKind::Punct(_)
                        | TokenKind::Eof,
                        _,
                    ) => Literal::Null,
                };
                ExpressionKind::Literal(literal)
            }
            NodeKind::Unary => match node.children.as_slice() {
                [Element::Token(minus), Element::Node(literal)]
                    if &self.source[minus.start..minus.end] == "-"
                        && matches!(
                            literal.children.as_slice(),
                            [Element::Token(Token { kind: TokenKind::IntegerLiteral, start, .. })]
                                if matches!(self.source.as_bytes()[*start], b'1'..=b'9')
                        ) =>
                {
                    self.expression_kind(literal)
                }
                _ => operator(NodeKind::Unary),
            },
            NodeKind::Binary | NodeKind::Assignment | NodeKind::Postfix => operator(node.kind),
            NodeKind::Name => ExpressionKind::Identifier,
            NodeKind::This | NodeKind::Super => match node.children.as_slice() {
                [Element::Token(_)] => ExpressionKind::Identifier,
                _ => ExpressionKind::MemberSelect,
            },
            NodeKind::FieldAccess | NodeKind::ClassLiteral => ExpressionKind::MemberSelect,
            NodeKind::ArrayInitializer | NodeKind::ElementValueArray | NodeKind::NewArray => {
                ExpressionKind::NewArray
            }
            NodeKind::CompilationUnit
            | NodeKind::PackageDecl
            | NodeKind::ImportDecl
            | NodeKind::ModuleDecl
            | NodeKind::ModuleDirective
            | NodeKind::QualifiedName
            | NodeKind::Modifiers
            | NodeKind::Annotation
            | NodeKind::AnnotationArguments
            | NodeKind::ElementValuePair
            | NodeKind::ClassDecl
            | NodeKind::InterfaceDecl
            | NodeKind::EnumDecl
            | NodeKind::RecordDecl
            | NodeKind::AnnotationTypeDecl
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
            | NodeKind::MethodDecl
            | NodeKind::ConstructorDecl
            | NodeKind::CompactConstructorDecl
            | NodeKind::FormalParameters
            | NodeKind::FormalParameter
            | NodeKind::ReceiverParameter
            | NodeKind::ThrowsClause
            | NodeKind::DefaultValue
            | NodeKind::Initializer
            | NodeKind::EmptyDecl
            | NodeKind::PrimitiveType
            | NodeKind::ClassType
            | NodeKind::ArrayType
            | NodeKind::Dimension
            | NodeKind::TypeArguments
            | NodeKind::Wildcard
            | NodeKind::VoidType
            | NodeKind::Block
            | NodeKind::LocalVariableDecl
            | NodeKind::LocalClassDecl
            | NodeKind::ExpressionStmt
            | NodeKind::IfStmt
            | NodeKind::WhileStmt
            | NodeKind::DoStmt
            | NodeKind::ForStmt
            | NodeKind::ForInit
            | NodeKind::ForUpdate
            | NodeKind::ForEachStmt
            | NodeKind::TryStmt
            | NodeKind::ResourceSpecification
            | NodeKind::Resource
            | NodeKind::CatchClause
            | NodeKind::CatchType
            | NodeKind::FinallyClause
            | NodeKind::SwitchStmt
            | NodeKind::SwitchBlock
            | NodeKind::SwitchRule
            | NodeKind::SwitchGroup
            | NodeKind::SwitchLabel
            | NodeKind::Guard
            | NodeKind::ReturnStmt
            | NodeKind::BreakStmt
            | NodeKind::ContinueStmt
            | NodeKind::ThrowStmt
            | NodeKind::YieldStmt
            | NodeKind::SynchronizedStmt
            | NodeKind::LabeledStmt
            | NodeKind::AssertStmt
            | NodeKind::EmptyStmt
            | NodeKind::MethodCall
            | NodeKind::Arguments
            | NodeKind::NewClass
            | NodeKind::ArrayAccess
            | NodeKind::InstanceOf
            | NodeKind::Conditional
            | NodeKind::Lambda
            | NodeKind::LambdaParameters
            | NodeKind::MethodReference
            | NodeKind::Cast
            | NodeKind::Parenthesized
            | NodeKind::SwitchExpr
            | NodeKind::TypePattern
            | NodeKind::RecordPattern
            | NodeKind::MatchAllPattern => ExpressionKind::Other(node.kind),
        }
    }
}
