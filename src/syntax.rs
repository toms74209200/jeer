use crate::lexer::token::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    CompilationUnit,
    PackageDecl,
    ImportDecl,
    ModuleDecl,
    ModuleDirective,
    QualifiedName,
    Modifiers,
    Annotation,
    AnnotationArguments,
    ElementValuePair,
    ElementValueArray,
    ClassDecl,
    InterfaceDecl,
    EnumDecl,
    RecordDecl,
    AnnotationTypeDecl,
    TypeParameters,
    TypeParameter,
    TypeBound,
    ExtendsClause,
    ImplementsClause,
    PermitsClause,
    ClassBody,
    EnumBody,
    EnumConstant,
    RecordHeader,
    RecordComponent,
    FieldDecl,
    VariableDeclarator,
    MethodDecl,
    ConstructorDecl,
    CompactConstructorDecl,
    FormalParameters,
    FormalParameter,
    ReceiverParameter,
    ThrowsClause,
    DefaultValue,
    Initializer,
    EmptyDecl,
    PrimitiveType,
    ClassType,
    ArrayType,
    Dimension,
    TypeArguments,
    Wildcard,
    VoidType,
    Block,
    LocalVariableDecl,
    LocalClassDecl,
    ExpressionStmt,
    IfStmt,
    WhileStmt,
    DoStmt,
    ForStmt,
    ForInit,
    ForUpdate,
    ForEachStmt,
    TryStmt,
    ResourceSpecification,
    Resource,
    CatchClause,
    CatchType,
    FinallyClause,
    SwitchStmt,
    SwitchBlock,
    SwitchRule,
    SwitchGroup,
    SwitchLabel,
    Guard,
    ReturnStmt,
    BreakStmt,
    ContinueStmt,
    ThrowStmt,
    YieldStmt,
    SynchronizedStmt,
    LabeledStmt,
    AssertStmt,
    EmptyStmt,
    Literal,
    Name,
    FieldAccess,
    MethodCall,
    Arguments,
    NewClass,
    NewArray,
    ArrayInitializer,
    ArrayAccess,
    Unary,
    Postfix,
    Binary,
    InstanceOf,
    Conditional,
    Assignment,
    Lambda,
    LambdaParameters,
    MethodReference,
    Cast,
    Parenthesized,
    ClassLiteral,
    This,
    Super,
    SwitchExpr,
    TypePattern,
    RecordPattern,
    MatchAllPattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    Node(Node),
    Token(Token),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<Element>,
}

impl Node {
    pub fn tokens(&self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut pending: Vec<&Element> = self.children.iter().rev().collect();
        while let Some(element) = pending.pop() {
            match element {
                Element::Token(token) => tokens.push(*token),
                Element::Node(node) => pending.extend(node.children.iter().rev()),
            }
        }
        tokens
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:?}", self.kind)?;
        for child in &self.children {
            if let Element::Node(node) = child {
                write!(f, " {node}")?;
            }
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::TokenKind;

    #[test]
    fn when_display_with_nested_nodes_then_returns_their_kinds_without_tokens() {
        let token = Token {
            kind: TokenKind::Identifier,
            start: 0,
            end: 1,
        };
        let node = Node {
            kind: NodeKind::Block,
            children: vec![
                Element::Token(token),
                Element::Node(Node {
                    kind: NodeKind::EmptyStmt,
                    children: vec![Element::Token(token)],
                }),
            ],
        };

        let shown = node.to_string();

        assert_eq!(shown, "(Block (EmptyStmt))");
    }
}
