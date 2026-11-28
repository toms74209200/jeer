mod declarations;
mod expressions;
mod statements;
pub mod syntax;
mod types;

use crate::lexer;
use crate::lexer::token::{Keyword, Punct, Token, TokenClass, TokenKind};
use crate::lexer::unicode_escapes::Translated;
use crate::parser::syntax::{Element, Node, NodeKind};

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

type Parsed = Result<Element, ParseError>;

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    significant: Vec<usize>,
    closing: Vec<Option<usize>>,
    pos: usize,
    raw: usize,
    split: Option<Token>,
}

pub struct Unit {
    pub tree: Node,
    pub text: Translated,
}

pub fn parse(source: &str) -> Result<Unit, ParseError> {
    let lexer::Lexed { tokens, text } = lexer::scan(source).map_err(|e| ParseError {
        offset: e.offset,
        message: e.kind.to_string(),
    })?;
    let significant = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.kind.class(), TokenClass::Significant))
        .map(|(i, _)| i)
        .collect::<Vec<usize>>();
    let mut closing = vec![None; significant.len()];
    let mut open = Vec::new();
    for (position, &index) in significant.iter().enumerate() {
        match tokens[index].kind {
            TokenKind::Punct(Punct::LParen) => open.push(position),
            TokenKind::Punct(Punct::RParen) => {
                if let Some(start) = open.pop() {
                    closing[start] = Some(position);
                }
            }
            _ => {}
        }
    }
    let mut parser = Parser {
        source,
        tokens,
        significant,
        closing,
        pos: 0,
        raw: 0,
        split: None,
    };
    let tree = parser.compilation_unit()?;
    Ok(Unit { tree, text })
}

impl Parser<'_> {
    fn token(&self, n: usize) -> Option<Token> {
        match (n, self.split) {
            (0, Some(rest)) => Some(rest),
            _ => self.significant.get(self.pos + n).map(|&i| self.tokens[i]),
        }
    }

    fn kind(&self, n: usize) -> TokenKind {
        match self.token(n) {
            Some(token) => token.kind,
            None => TokenKind::Eof,
        }
    }

    fn text(&self, n: usize) -> &str {
        match self.token(n) {
            Some(token) => &self.source[token.start..token.end],
            None => "",
        }
    }

    fn fail<T>(&self, expected: &str) -> Result<T, ParseError> {
        let offset = self.token(0).map_or(self.source.len(), |t| t.start);
        Err(ParseError {
            offset,
            message: expected.to_owned(),
        })
    }

    fn missing<T>(&self, expected: &str) -> Result<T, ParseError> {
        let offset = self.split.map_or_else(
            || {
                self.pos
                    .checked_sub(1)
                    .and_then(|previous| self.significant.get(previous))
                    .and_then(|&index| self.tokens.get(index))
                    .map_or(0, |token| token.end)
            },
            |rest| rest.start,
        );
        Err(ParseError {
            offset,
            message: expected.to_owned(),
        })
    }

    fn trivia(&mut self, out: &mut Vec<Element>) {
        let index = self
            .significant
            .get(self.pos)
            .copied()
            .unwrap_or(self.tokens.len());
        let pending = self.tokens.get(self.raw..index).unwrap_or_default();
        out.extend(pending.iter().map(|t| Element::Token(*t)));
        self.raw = self.raw.max(index);
    }

    fn bump(&mut self, out: &mut Vec<Element>) {
        self.trivia(out);
        let current = self.split.take().or_else(|| self.token(0));
        if let Some(token) = current {
            out.push(Element::Token(token));
            self.raw += 1;
            self.pos += 1;
        }
    }

    fn expect(&mut self, kind: TokenKind, out: &mut Vec<Element>) -> Result<(), ParseError> {
        match self.kind(0) == kind {
            true => {
                self.bump(out);
                Ok(())
            }
            false => self.missing(&format!("{kind:?} expected")),
        }
    }

    fn expect_identifier(
        &mut self,
        out: &mut Vec<Element>,
        absent: fn(&Self, &str) -> Result<(), ParseError>,
    ) -> Result<(), ParseError> {
        match self.kind(0) {
            TokenKind::Identifier | TokenKind::Keyword(Keyword::Underscore) => {
                self.bump(out);
                Ok(())
            }
            _ => absent(self, "<identifier> expected"),
        }
    }

    fn close_angle(&mut self, out: &mut Vec<Element>) -> Result<(), ParseError> {
        let rest = match self.kind(0) {
            TokenKind::Punct(Punct::Gt) => None,
            TokenKind::Punct(Punct::GtGt) => Some(TokenKind::Punct(Punct::Gt)),
            TokenKind::Punct(Punct::GtGtGt) => Some(TokenKind::Punct(Punct::GtGt)),
            TokenKind::Punct(Punct::GtEq) => Some(TokenKind::Punct(Punct::Assign)),
            TokenKind::Punct(Punct::GtGtEq) => Some(TokenKind::Punct(Punct::GtEq)),
            TokenKind::Punct(Punct::GtGtGtEq) => Some(TokenKind::Punct(Punct::GtGtEq)),
            _ => return self.fail("'>' expected"),
        };
        match (rest, self.token(0)) {
            (Some(kind), Some(token)) => {
                self.trivia(out);
                out.push(Element::Token(Token {
                    kind: TokenKind::Punct(Punct::Gt),
                    start: token.start,
                    end: token.start + 1,
                }));
                self.split = Some(Token {
                    kind,
                    start: token.start + 1,
                    end: token.end,
                });
            }
            _ => self.bump(out),
        }
        Ok(())
    }

    fn probe<T>(
        &mut self,
        attempt: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let (pos, raw, split) = (self.pos, self.raw, self.split);
        let result = attempt(self);
        self.pos = pos;
        self.raw = raw;
        self.split = split;
        result
    }

    fn qualified_name(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.expect_identifier(&mut children, Self::missing)?;
        while let (TokenKind::Punct(Punct::Dot), TokenKind::Identifier) =
            (self.kind(0), self.kind(1))
        {
            self.bump(&mut children);
            self.bump(&mut children);
        }
        Ok(Element::Node(Node {
            kind: NodeKind::QualifiedName,
            children,
        }))
    }

    fn compilation_unit(&mut self) -> Result<Node, ParseError> {
        let mut children = Vec::new();
        let package = self.probe(|p| {
            let mut ignored = Vec::new();
            p.modifiers(&mut ignored)?;
            Ok(p.kind(0))
        });
        if let Ok(TokenKind::Keyword(Keyword::Package)) = package {
            let mut package = Vec::new();
            self.modifiers(&mut package)?;
            self.bump(&mut package);
            package.push(self.qualified_name()?);
            self.expect(TokenKind::Punct(Punct::Semicolon), &mut package)?;
            children.push(Element::Node(Node {
                kind: NodeKind::PackageDecl,
                children: package,
            }));
        }
        let mut section = Section::Imports;
        loop {
            match (self.kind(0), &section) {
                (TokenKind::Keyword(Keyword::Import), Section::Imports) => {
                    children.push(self.import()?)
                }
                (TokenKind::Keyword(Keyword::Import), Section::Declarations) => {
                    return self.fail("class, interface, enum, or record expected");
                }
                (TokenKind::Punct(Punct::Semicolon), _) => {
                    let mut empty = Vec::new();
                    self.bump(&mut empty);
                    children.push(Element::Node(Node {
                        kind: NodeKind::EmptyDecl,
                        children: empty,
                    }));
                }
                (TokenKind::Eof, _) => break,
                _ => {
                    section = Section::Declarations;
                    let module = self.probe(|p| {
                        let mut ignored = Vec::new();
                        p.modifiers(&mut ignored)?;
                        Ok((p.text(0).to_owned(), p.kind(1), p.text(1).to_owned()))
                    });
                    match module {
                        Ok((first, TokenKind::Identifier, _)) if first == "module" => {
                            children.push(self.module()?)
                        }
                        Ok((first, TokenKind::Identifier, second))
                            if first == "open" && second == "module" =>
                        {
                            children.push(self.module()?);
                        }
                        _ => children.push(self.member(TypeContext::Other)?),
                    }
                }
            }
        }
        let rest = self.tokens.get(self.raw..).unwrap_or_default();
        children.extend(rest.iter().map(|t| Element::Token(*t)));
        Ok(Node {
            kind: NodeKind::CompilationUnit,
            children,
        })
    }

    fn import(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.bump(&mut children);
        match (self.kind(0), self.text(0), self.kind(1)) {
            (TokenKind::Keyword(Keyword::Static), _, _) => self.bump(&mut children),
            (TokenKind::Identifier, "module", TokenKind::Identifier) => self.bump(&mut children),
            _ => {}
        }
        self.expect_identifier(&mut children, Self::missing)?;
        loop {
            match (self.kind(0), self.kind(1)) {
                (TokenKind::Punct(Punct::Dot), TokenKind::Identifier) => {
                    self.bump(&mut children);
                    self.bump(&mut children);
                }
                (TokenKind::Punct(Punct::Dot), TokenKind::Punct(Punct::Star)) => {
                    self.bump(&mut children);
                    self.bump(&mut children);
                    break;
                }
                _ => break,
            }
        }
        self.expect(TokenKind::Punct(Punct::Semicolon), &mut children)?;
        Ok(Element::Node(Node {
            kind: NodeKind::ImportDecl,
            children,
        }))
    }

    fn module(&mut self) -> Parsed {
        let mut children = Vec::new();
        self.modifiers(&mut children)?;
        if self.text(0) == "open" {
            self.bump(&mut children);
        }
        self.bump(&mut children);
        children.push(self.qualified_name()?);
        self.expect(TokenKind::Punct(Punct::LBrace), &mut children)?;
        while self.kind(0) != TokenKind::Punct(Punct::RBrace) {
            let mut directive = Vec::new();
            match (self.kind(0), self.text(0)) {
                (TokenKind::Identifier, "requires") => {
                    self.bump(&mut directive);
                    while let (
                        "transitive" | "static",
                        TokenKind::Identifier | TokenKind::Keyword(Keyword::Static),
                    ) = (self.text(0), self.kind(1))
                    {
                        self.bump(&mut directive);
                    }
                    directive.push(self.qualified_name()?);
                }
                (TokenKind::Identifier, "exports" | "opens") => {
                    self.bump(&mut directive);
                    directive.push(self.qualified_name()?);
                    if self.text(0) == "to" {
                        self.bump(&mut directive);
                        directive.push(self.qualified_name()?);
                        while self.kind(0) == TokenKind::Punct(Punct::Comma) {
                            self.bump(&mut directive);
                            directive.push(self.qualified_name()?);
                        }
                    }
                }
                (TokenKind::Identifier, "uses") => {
                    self.bump(&mut directive);
                    directive.push(self.qualified_name()?);
                }
                (TokenKind::Identifier, "provides") => {
                    self.bump(&mut directive);
                    directive.push(self.qualified_name()?);
                    match self.text(0) {
                        "with" => self.bump(&mut directive),
                        _ => return self.fail("'with' expected"),
                    }
                    directive.push(self.qualified_name()?);
                    while self.kind(0) == TokenKind::Punct(Punct::Comma) {
                        self.bump(&mut directive);
                        directive.push(self.qualified_name()?);
                    }
                }
                _ => return self.fail("module directive expected"),
            }
            self.expect(TokenKind::Punct(Punct::Semicolon), &mut directive)?;
            children.push(Element::Node(Node {
                kind: NodeKind::ModuleDirective,
                children: directive,
            }));
        }
        self.bump(&mut children);
        Ok(Element::Node(Node {
            kind: NodeKind::ModuleDecl,
            children,
        }))
    }
}

enum Section {
    Imports,
    Declarations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeContext {
    Record,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_with_each_construct_then_returns_its_tree_reproducing_the_source() {
        let cases = [
            (
                r#"package a.b; import java.util.List; import static a.B.c; import a.*; import module java.base; class A {}"#,
                r#"(CompilationUnit (PackageDecl (Modifiers) (QualifiedName)) (ImportDecl) (ImportDecl) (ImportDecl) (ImportDecl) (ClassDecl (Modifiers) (ClassBody)))"#,
            ),
            (
                r#"@Deprecated package a;"#,
                r#"(CompilationUnit (PackageDecl (Modifiers (Annotation (QualifiedName))) (QualifiedName)))"#,
            ),
            (
                r#"module a.b { requires transitive static c; requires d.e; exports p; exports q to r, s; opens t to u; uses V; provides W with X, Y; }"#,
                r#"(CompilationUnit (ModuleDecl (Modifiers) (QualifiedName) (ModuleDirective (QualifiedName)) (ModuleDirective (QualifiedName)) (ModuleDirective (QualifiedName)) (ModuleDirective (QualifiedName) (QualifiedName) (QualifiedName)) (ModuleDirective (QualifiedName) (QualifiedName)) (ModuleDirective (QualifiedName)) (ModuleDirective (QualifiedName) (QualifiedName) (QualifiedName))))"#,
            ),
            (
                r#"open module m {}"#,
                r#"(CompilationUnit (ModuleDecl (Modifiers) (QualifiedName)))"#,
            ),
            (
                r#"public final class A<T extends B & C, U> extends D<T> implements E, F permits G, H { }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (TypeParameters (TypeParameter (Modifiers) (TypeBound (ClassType) (ClassType))) (TypeParameter (Modifiers))) (ExtendsClause (ClassType (TypeArguments (ClassType)))) (ImplementsClause (ClassType) (ClassType)) (PermitsClause (ClassType) (ClassType)) (ClassBody)))"#,
            ),
            (
                r#"sealed interface I permits J {} non-sealed class J implements I {}"#,
                r#"(CompilationUnit (InterfaceDecl (Modifiers) (PermitsClause (ClassType)) (ClassBody)) (ClassDecl (Modifiers) (ImplementsClause (ClassType)) (ClassBody)))"#,
            ),
            (
                r#"interface I<T> extends J, K { default void f() {} static int g() { return 1; } void h(); }"#,
                r#"(CompilationUnit (InterfaceDecl (Modifiers) (TypeParameters (TypeParameter (Modifiers))) (ExtendsClause (ClassType) (ClassType)) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block)) (MethodDecl (Modifiers) (PrimitiveType) (FormalParameters) (Block (ReturnStmt (Literal)))) (MethodDecl (Modifiers) (VoidType) (FormalParameters)))))"#,
            ),
            (
                r#"enum E implements I { A, @Deprecated B(1), C { void f() {} }; private int x; E() {} E(int x) { this.x = x; } }"#,
                r#"(CompilationUnit (EnumDecl (Modifiers) (ImplementsClause (ClassType)) (EnumBody (EnumConstant (Modifiers)) (EnumConstant (Modifiers (Annotation (QualifiedName))) (Arguments (Literal))) (EnumConstant (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block)))) (FieldDecl (Modifiers) (PrimitiveType) (VariableDeclarator)) (ConstructorDecl (Modifiers) (FormalParameters) (Block)) (ConstructorDecl (Modifiers) (FormalParameters (FormalParameter (Modifiers) (PrimitiveType))) (Block (ExpressionStmt (Assignment (FieldAccess (This)) (Name))))))))"#,
            ),
            (
                r#"enum F { A, B, }"#,
                r#"(CompilationUnit (EnumDecl (Modifiers) (EnumBody (EnumConstant (Modifiers)) (EnumConstant (Modifiers)))))"#,
            ),
            (
                r#"enum G { ; void f() {} }"#,
                r#"(CompilationUnit (EnumDecl (Modifiers) (EnumBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block)))))"#,
            ),
            (
                r#"record R<T>(int x, @A String... rest) implements I { R { } R(int x) { this(x, ""); } static int y; }"#,
                r#"(CompilationUnit (RecordDecl (Modifiers) (TypeParameters (TypeParameter (Modifiers))) (RecordHeader (RecordComponent (Modifiers) (PrimitiveType)) (RecordComponent (Modifiers (Annotation (QualifiedName))) (ClassType))) (ImplementsClause (ClassType)) (ClassBody (CompactConstructorDecl (Modifiers) (Block)) (ConstructorDecl (Modifiers) (FormalParameters (FormalParameter (Modifiers) (PrimitiveType))) (Block (ExpressionStmt (MethodCall (This) (Arguments (Name) (Literal)))))) (FieldDecl (Modifiers) (PrimitiveType) (VariableDeclarator)))))"#,
            ),
            (
                r#"@interface A { int value() default 1; String[] names() default {"a", "b"}; B nested() default @B; }"#,
                r#"(CompilationUnit (AnnotationTypeDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (PrimitiveType) (FormalParameters) (DefaultValue (Literal))) (MethodDecl (Modifiers) (ArrayType (ClassType) (Dimension)) (FormalParameters) (DefaultValue (ElementValueArray (Literal) (Literal)))) (MethodDecl (Modifiers) (ClassType) (FormalParameters) (DefaultValue (Annotation (QualifiedName)))))))"#,
            ),
            (
                r#"class A { int x = 1, y[] = {1, 2}, z; static { } { } A() {} <T> A(T t) throws E, F {} <T extends Comparable<? super T>> void f(T... ts) {} int g()[] { return null; } abstract void h(); }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (FieldDecl (Modifiers) (PrimitiveType) (VariableDeclarator (Literal)) (VariableDeclarator (Dimension) (ArrayInitializer (Literal) (Literal))) (VariableDeclarator)) (Initializer (Modifiers) (Block)) (Initializer (Modifiers) (Block)) (ConstructorDecl (Modifiers) (FormalParameters) (Block)) (ConstructorDecl (Modifiers) (TypeParameters (TypeParameter (Modifiers))) (FormalParameters (FormalParameter (Modifiers) (ClassType))) (ThrowsClause (ClassType) (ClassType)) (Block)) (MethodDecl (Modifiers) (TypeParameters (TypeParameter (Modifiers) (TypeBound (ClassType (TypeArguments (Wildcard (ClassType))))))) (VoidType) (FormalParameters (FormalParameter (Modifiers) (ClassType))) (Block)) (MethodDecl (Modifiers) (PrimitiveType) (FormalParameters) (Dimension) (Block (ReturnStmt (Literal)))) (MethodDecl (Modifiers) (VoidType) (FormalParameters)))))"#,
            ),
            (
                r#"class A { void f(A this) {} class B { B(A A.this) {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters (ReceiverParameter (Modifiers) (ClassType))) (Block)) (ClassDecl (Modifiers) (ClassBody (ConstructorDecl (Modifiers) (FormalParameters (ReceiverParameter (Modifiers) (ClassType))) (Block)))))))"#,
            ),
            (
                r#"class A { @SuppressWarnings({"a", "b"}) @B(x = 1, y = "s") @C(D.E + "/f", g = H.class) void f(@A final int a, int @B [] b) {} }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers (Annotation (QualifiedName) (AnnotationArguments (ElementValueArray (Literal) (Literal)))) (Annotation (QualifiedName) (AnnotationArguments (ElementValuePair (Literal)) (ElementValuePair (Literal)))) (Annotation (QualifiedName) (AnnotationArguments (Binary (FieldAccess (Name)) (Literal)) (ElementValuePair (ClassLiteral (ClassType)))))) (VoidType) (FormalParameters (FormalParameter (Modifiers (Annotation (QualifiedName))) (PrimitiveType)) (FormalParameter (Modifiers) (ArrayType (PrimitiveType) (Dimension (Annotation (QualifiedName)))))) (Block)))))"#,
            ),
            (
                r#"class A { List<? extends Number> a; Map<String, List<int[]>> b; Map<?, ?> c; java.util.@A List<@B String> d; int @A [] @B [] e; a.b.C<D>.E<F> g; }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (FieldDecl (Modifiers) (ClassType (TypeArguments (Wildcard (ClassType)))) (VariableDeclarator)) (FieldDecl (Modifiers) (ClassType (TypeArguments (ClassType) (ClassType (TypeArguments (ArrayType (PrimitiveType) (Dimension)))))) (VariableDeclarator)) (FieldDecl (Modifiers) (ClassType (TypeArguments (Wildcard) (Wildcard))) (VariableDeclarator)) (FieldDecl (Modifiers) (ClassType (Annotation (QualifiedName)) (TypeArguments (ClassType (Annotation (QualifiedName))))) (VariableDeclarator)) (FieldDecl (Modifiers) (ArrayType (PrimitiveType) (Dimension (Annotation (QualifiedName))) (Dimension (Annotation (QualifiedName)))) (VariableDeclarator)) (FieldDecl (Modifiers) (ClassType (TypeArguments (ClassType)) (TypeArguments (ClassType))) (VariableDeclarator)))))"#,
            ),
            (
                r#"class A { void f() { int a; final var b = 1; List<String> c = new ArrayList<>(); int[] d = new int[3]; int e[] = new int[] {1}; long[][] f = new long[2][]; String g = new String @A [2] @B []; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalVariableDecl (Modifiers) (PrimitiveType) (VariableDeclarator)) (LocalVariableDecl (Modifiers) (ClassType) (VariableDeclarator (Literal))) (LocalVariableDecl (Modifiers) (ClassType (TypeArguments (ClassType))) (VariableDeclarator (NewClass (ClassType (TypeArguments)) (Arguments)))) (LocalVariableDecl (Modifiers) (ArrayType (PrimitiveType) (Dimension)) (VariableDeclarator (NewArray (PrimitiveType) (Dimension (Literal))))) (LocalVariableDecl (Modifiers) (PrimitiveType) (VariableDeclarator (Dimension) (NewArray (ArrayType (PrimitiveType) (Dimension)) (ArrayInitializer (Literal))))) (LocalVariableDecl (Modifiers) (ArrayType (PrimitiveType) (Dimension) (Dimension)) (VariableDeclarator (NewArray (PrimitiveType) (Dimension (Literal)) (Dimension)))) (LocalVariableDecl (Modifiers) (ClassType) (VariableDeclarator (NewArray (ClassType) (Dimension (Annotation (QualifiedName)) (Literal)) (Dimension (Annotation (QualifiedName)))))))))))"#,
            ),
            (
                r#"class A { void f() { class L {} record M(int x) {} enum N { X } interface O {} @interface P {} abstract static class Q {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalClassDecl (ClassDecl (Modifiers) (ClassBody))) (LocalClassDecl (RecordDecl (Modifiers) (RecordHeader (RecordComponent (Modifiers) (PrimitiveType))) (ClassBody))) (LocalClassDecl (EnumDecl (Modifiers) (EnumBody (EnumConstant (Modifiers))))) (LocalClassDecl (InterfaceDecl (Modifiers) (ClassBody))) (LocalClassDecl (AnnotationTypeDecl (Modifiers) (ClassBody))) (LocalClassDecl (ClassDecl (Modifiers) (ClassBody))))))))"#,
            ),
            (
                r#"class A { void f() { if (a) b(); else if (c) { d(); } else e(); while (x) y(); do z(); while (w); for (int i = 0, j = 1; i < j; i++, j--) {} for (;;) break; for (i = 0; ; ) continue; for (var x : xs) {} for (final int @A [] y : ys) {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (IfStmt (Parenthesized (Name)) (ExpressionStmt (MethodCall (Name) (Arguments))) (IfStmt (Parenthesized (Name)) (Block (ExpressionStmt (MethodCall (Name) (Arguments)))) (ExpressionStmt (MethodCall (Name) (Arguments))))) (WhileStmt (Parenthesized (Name)) (ExpressionStmt (MethodCall (Name) (Arguments)))) (DoStmt (ExpressionStmt (MethodCall (Name) (Arguments))) (Parenthesized (Name))) (ForStmt (ForInit (LocalVariableDecl (Modifiers) (PrimitiveType) (VariableDeclarator (Literal)) (VariableDeclarator (Literal)))) (Binary (Name) (Name)) (ForUpdate (Postfix (Name)) (Postfix (Name))) (Block)) (ForStmt (ForInit) (ForUpdate) (BreakStmt)) (ForStmt (ForInit (Assignment (Name) (Literal))) (ForUpdate) (ContinueStmt)) (ForEachStmt (LocalVariableDecl (Modifiers) (ClassType)) (Name) (Block)) (ForEachStmt (LocalVariableDecl (Modifiers) (ArrayType (PrimitiveType) (Dimension (Annotation (QualifiedName))))) (Name) (Block)))))))"#,
            ),
            (
                r#"class A { void f() { try { a(); } catch (final E | F e) { } catch (G _) { } finally { b(); } try (var x = open(); Y y = z(); w; this.v) { } } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (TryStmt (Block (ExpressionStmt (MethodCall (Name) (Arguments)))) (CatchClause (Modifiers) (CatchType (ClassType) (ClassType)) (Block)) (CatchClause (Modifiers) (CatchType (ClassType)) (Block)) (FinallyClause (Block (ExpressionStmt (MethodCall (Name) (Arguments)))))) (TryStmt (ResourceSpecification (Resource (Modifiers) (ClassType) (MethodCall (Name) (Arguments))) (Resource (Modifiers) (ClassType) (MethodCall (Name) (Arguments))) (Resource (Name)) (Resource (FieldAccess (This)))) (Block)))))))"#,
            ),
            (
                r#"class A { void f() { switch (x) { case 1: case 2, 3: a(); break; default: b(); } switch (y) { case A -> a(); case B, C -> { b(); } default -> throw new E(); } } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (SwitchStmt (Parenthesized (Name)) (SwitchBlock (SwitchGroup (SwitchLabel (Literal))) (SwitchGroup (SwitchLabel (Literal) (Literal)) (ExpressionStmt (MethodCall (Name) (Arguments))) (BreakStmt)) (SwitchGroup (SwitchLabel) (ExpressionStmt (MethodCall (Name) (Arguments)))))) (SwitchStmt (Parenthesized (Name)) (SwitchBlock (SwitchRule (SwitchLabel (Name)) (ExpressionStmt (MethodCall (Name) (Arguments)))) (SwitchRule (SwitchLabel (Name) (Name)) (Block (ExpressionStmt (MethodCall (Name) (Arguments))))) (SwitchRule (SwitchLabel) (ThrowStmt (NewClass (ClassType) (Arguments)))))))))))"#,
            ),
            (
                r#"class A { int f(Object o) { return switch (o) { case null, default -> 0; case Integer i when i > 0 -> i; case String s -> s.length(); case Point(int x, var y) -> x; case Box(Point(var a, _), _) -> 1; case E.A -> 2; default -> { yield 3; } }; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (PrimitiveType) (FormalParameters (FormalParameter (Modifiers) (ClassType))) (Block (ReturnStmt (SwitchExpr (Name) (SwitchBlock (SwitchRule (SwitchLabel (Literal)) (ExpressionStmt (Literal))) (SwitchRule (SwitchLabel (TypePattern (Modifiers) (ClassType)) (Guard (Binary (Name) (Literal)))) (ExpressionStmt (Name))) (SwitchRule (SwitchLabel (TypePattern (Modifiers) (ClassType))) (ExpressionStmt (MethodCall (Name) (Arguments)))) (SwitchRule (SwitchLabel (RecordPattern (Modifiers) (ClassType) (TypePattern (Modifiers) (PrimitiveType)) (TypePattern (Modifiers) (ClassType)))) (ExpressionStmt (Name))) (SwitchRule (SwitchLabel (RecordPattern (Modifiers) (ClassType) (RecordPattern (Modifiers) (ClassType) (TypePattern (Modifiers) (ClassType)) (MatchAllPattern)) (MatchAllPattern))) (ExpressionStmt (Literal))) (SwitchRule (SwitchLabel (FieldAccess (Name))) (ExpressionStmt (Literal))) (SwitchRule (SwitchLabel) (Block (YieldStmt (Literal))))))))))))"#,
            ),
            (
                r#"class A { void f() { return; label: for (;;) { break label; continue label; } throw e; synchronized (this) { } assert a; assert b : "c"; yield 1; yield = 2; ; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ReturnStmt) (LabeledStmt (ForStmt (ForInit) (ForUpdate) (Block (BreakStmt) (ContinueStmt)))) (ThrowStmt (Name)) (SynchronizedStmt (Parenthesized (This)) (Block)) (AssertStmt (Name)) (AssertStmt (Name) (Literal)) (YieldStmt (Literal)) (ExpressionStmt (Assignment (Name) (Literal))) (EmptyStmt))))))"#,
            ),
            (
                r#"class A { void f() { a = b += c -= d; x = y ? z : w ? v : u; a = b || c && d | e ^ f & g == h != i < j > k <= l >= m << n >> o >>> p + q - r * s / t % u; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Assignment (Name) (Assignment (Name) (Name))))) (ExpressionStmt (Assignment (Name) (Conditional (Name) (Name) (Conditional (Name) (Name) (Name))))) (ExpressionStmt (Assignment (Name) (Binary (Name) (Binary (Name) (Binary (Name) (Binary (Name) (Binary (Name) (Binary (Binary (Name) (Name)) (Binary (Binary (Binary (Binary (Name) (Name)) (Name)) (Name)) (Binary (Binary (Binary (Name) (Name)) (Name)) (Binary (Binary (Name) (Name)) (Binary (Binary (Binary (Name) (Name)) (Name)) (Name))))))))))))))))))"#,
            ),
            (
                r#"class A { void f() { a = -b + +c - ++d * --e / !f % ~g; h++; i--; j = (int) k; l = (String) m; n = (List<String>) o; p = (A & B) q; r = (int) -s; t = (u) + v; w = (x); y = (z) -> 1; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Binary (Binary (Unary (Name)) (Unary (Name))) (Binary (Binary (Binary (Unary (Name)) (Unary (Name))) (Unary (Name))) (Unary (Name)))))) (ExpressionStmt (Postfix (Name))) (ExpressionStmt (Postfix (Name))) (ExpressionStmt (Assignment (Name) (Cast (PrimitiveType) (Name)))) (ExpressionStmt (Assignment (Name) (Cast (ClassType) (Name)))) (ExpressionStmt (Assignment (Name) (Cast (ClassType (TypeArguments (ClassType))) (Name)))) (ExpressionStmt (Assignment (Name) (Cast (ClassType) (ClassType) (Name)))) (ExpressionStmt (Assignment (Name) (Cast (PrimitiveType) (Unary (Name))))) (ExpressionStmt (Assignment (Name) (Binary (Parenthesized (Name)) (Name)))) (ExpressionStmt (Assignment (Name) (Parenthesized (Name)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters) (Literal)))))))))"#,
            ),
            (
                r#"class A { void f() { a = b instanceof C; d = e instanceof final F f; g = h instanceof I(var j, K k); l = m instanceof N<O> p && p.q(); } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (InstanceOf (Name) (Modifiers) (ClassType)))) (ExpressionStmt (Assignment (Name) (InstanceOf (Name) (TypePattern (Modifiers) (ClassType))))) (ExpressionStmt (Assignment (Name) (InstanceOf (Name) (RecordPattern (Modifiers) (ClassType) (TypePattern (Modifiers) (ClassType)) (TypePattern (Modifiers) (ClassType)))))) (ExpressionStmt (Assignment (Name) (Binary (InstanceOf (Name) (TypePattern (Modifiers) (ClassType (TypeArguments (ClassType))))) (MethodCall (Name) (Arguments))))))))))"#,
            ),
            (
                r#"class A { void f() { r = () -> {}; s = x -> x; t = (x, y) -> x + y; u = (int x, final String y) -> x; v = (var a, var b) -> a; w = _ -> 1; z = (String... args) -> 0; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters) (Block)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters) (Name)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters) (Binary (Name) (Name))))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters (FormalParameter (Modifiers) (PrimitiveType)) (FormalParameter (Modifiers) (ClassType))) (Name)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters (FormalParameter (Modifiers) (ClassType)) (FormalParameter (Modifiers) (ClassType))) (Name)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters) (Literal)))) (ExpressionStmt (Assignment (Name) (Lambda (LambdaParameters (FormalParameter (Modifiers) (ClassType))) (Literal)))))))))"#,
            ),
            (
                r#"class A { void f() { a = b.c.d; e = f.g(h, i); j = k[l][m]; n = this.o; p = super.q(); r = A.this.s; t = A.super.u(); v = w.<X>y(); z = new B<>() { }; c = d.new E(); f = int.class; g = String[].class; h = void.class; i = A.class; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (FieldAccess (FieldAccess (Name))))) (ExpressionStmt (Assignment (Name) (MethodCall (Name) (Arguments (Name) (Name))))) (ExpressionStmt (Assignment (Name) (ArrayAccess (ArrayAccess (Name) (Name)) (Name)))) (ExpressionStmt (Assignment (Name) (FieldAccess (This)))) (ExpressionStmt (Assignment (Name) (MethodCall (Super) (Arguments)))) (ExpressionStmt (Assignment (Name) (FieldAccess (This (Name))))) (ExpressionStmt (Assignment (Name) (MethodCall (Super (Name)) (Arguments)))) (ExpressionStmt (Assignment (Name) (MethodCall (Name) (TypeArguments (ClassType)) (Arguments)))) (ExpressionStmt (Assignment (Name) (NewClass (ClassType (TypeArguments)) (Arguments) (ClassBody)))) (ExpressionStmt (Assignment (Name) (NewClass (Name) (ClassType) (Arguments)))) (ExpressionStmt (Assignment (Name) (ClassLiteral (PrimitiveType)))) (ExpressionStmt (Assignment (Name) (ClassLiteral (ArrayType (ClassType) (Dimension))))) (ExpressionStmt (Assignment (Name) (ClassLiteral (VoidType)))) (ExpressionStmt (Assignment (Name) (ClassLiteral (ClassType)))))))))"#,
            ),
            (
                r#"class A { A() { this(1); } A(int x) { super(); } A(long y) { <T>this(); } void f() { g = List<String>::size; h = String[]::new; i = int[]::new; j = this::k; l = super::m; n = A::<T>o; p = @B C::new; q = r::s; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (ConstructorDecl (Modifiers) (FormalParameters) (Block (ExpressionStmt (MethodCall (This) (Arguments (Literal)))))) (ConstructorDecl (Modifiers) (FormalParameters (FormalParameter (Modifiers) (PrimitiveType))) (Block (ExpressionStmt (MethodCall (Super) (Arguments))))) (ConstructorDecl (Modifiers) (FormalParameters (FormalParameter (Modifiers) (PrimitiveType))) (Block (ExpressionStmt (MethodCall (TypeArguments (ClassType)) (Arguments))))) (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (MethodReference (ClassType (TypeArguments (ClassType)))))) (ExpressionStmt (Assignment (Name) (MethodReference (ArrayType (ClassType) (Dimension))))) (ExpressionStmt (Assignment (Name) (MethodReference (ArrayType (PrimitiveType) (Dimension))))) (ExpressionStmt (Assignment (Name) (MethodReference (This)))) (ExpressionStmt (Assignment (Name) (MethodReference (Super)))) (ExpressionStmt (Assignment (Name) (MethodReference (ClassType) (TypeArguments (ClassType))))) (ExpressionStmt (Assignment (Name) (MethodReference (ClassType (Annotation (QualifiedName)))))) (ExpressionStmt (Assignment (Name) (MethodReference (ClassType)))))))))"#,
            ),
            (
                r#"class A { void f() { a = 1 + 2L + 3.0f + 'c' + "s" + """
    t
    """ + true + false + null; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Binary (Binary (Binary (Binary (Binary (Binary (Binary (Binary (Literal) (Literal)) (Literal)) (Literal)) (Literal)) (Literal)) (Literal)) (Literal)) (Literal)))))))))"#,
            ),
            (
                r#"int x = 1; void main() { }"#,
                r#"(CompilationUnit (FieldDecl (Modifiers) (PrimitiveType) (VariableDeclarator (Literal))) (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block)))"#,
            ),
            (
                r#"class A { void f() { java.util.List<String> a; a.b.C c = null; A<B>.C d; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalVariableDecl (Modifiers) (ClassType (TypeArguments (ClassType))) (VariableDeclarator)) (LocalVariableDecl (Modifiers) (ClassType) (VariableDeclarator (Literal))) (LocalVariableDecl (Modifiers) (ClassType (TypeArguments (ClassType))) (VariableDeclarator)))))))"#,
            ),
            (
                r#"class A { Map<String,List<String>> m; List<List<List<A>>> n; }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (FieldDecl (Modifiers) (ClassType (TypeArguments (ClassType) (ClassType (TypeArguments (ClassType))))) (VariableDeclarator)) (FieldDecl (Modifiers) (ClassType (TypeArguments (ClassType (TypeArguments (ClassType (TypeArguments (ClassType))))))) (VariableDeclarator)))))"#,
            ),
            (
                r#"class A { void f() { x = y >>= z >>>= w; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Assignment (Name) (Assignment (Name) (Name))))))))))"#,
            ),
            (
                r#"class A { void f(int... x) {} }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters (FormalParameter (Modifiers) (PrimitiveType))) (Block)))))"#,
            ),
            (
                r#"class A { void f() { if (x instanceof final A a) {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (IfStmt (Parenthesized (InstanceOf (Name) (TypePattern (Modifiers) (ClassType)))) (Block)))))))"#,
            ),
            (
                r#"class A { void f() { record R() {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalClassDecl (RecordDecl (Modifiers) (RecordHeader) (ClassBody))))))))"#,
            ),
            (
                r#"package a; ; ; class A {}"#,
                r#"(CompilationUnit (PackageDecl (Modifiers) (QualifiedName)) (EmptyDecl) (EmptyDecl) (ClassDecl (Modifiers) (ClassBody)))"#,
            ),
            (
                r#"class A { void f() { new A().<T>g(); } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (MethodCall (NewClass (ClassType) (Arguments)) (TypeArguments (ClassType)) (Arguments))))))))"#,
            ),
            (
                r#"class A { void f() { int[] a = {}; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalVariableDecl (Modifiers) (ArrayType (PrimitiveType) (Dimension)) (VariableDeclarator (ArrayInitializer))))))))"#,
            ),
            (
                r#"class A { void f() { x = cond ? a -> 1 : b -> 2; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Conditional (Name) (Lambda (LambdaParameters) (Literal)) (Lambda (LambdaParameters) (Literal))))))))))"#,
            ),
            (
                r#"class A { void f() { x = (Runnable) () -> {}; y = (int) (a) + b; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Cast (ClassType) (Lambda (LambdaParameters) (Block))))) (ExpressionStmt (Assignment (Name) (Binary (Cast (PrimitiveType) (Parenthesized (Name))) (Name)))))))))"#,
            ),
            (
                r#"class A { void f() { a = b < c; d = e > f; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ExpressionStmt (Assignment (Name) (Binary (Name) (Name)))) (ExpressionStmt (Assignment (Name) (Binary (Name) (Name)))))))))"#,
            ),
            (
                r#"interface I { default int x = 1; }"#,
                r#"(CompilationUnit (InterfaceDecl (Modifiers) (ClassBody (FieldDecl (Modifiers) (PrimitiveType) (VariableDeclarator (Literal))))))"#,
            ),
            (
                r#"class A { @Ann(x = {@B, @C}) int f; }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (FieldDecl (Modifiers (Annotation (QualifiedName) (AnnotationArguments (ElementValuePair (ElementValueArray (Annotation (QualifiedName)) (Annotation (QualifiedName))))))) (PrimitiveType) (VariableDeclarator)))))"#,
            ),
            (
                r#"class A { void f() { for (Map.Entry<K, V> e : m.entrySet()) {} } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (ForEachStmt (LocalVariableDecl (Modifiers) (ClassType (TypeArguments (ClassType) (ClassType)))) (MethodCall (Name) (Arguments)) (Block)))))))"#,
            ),
            (
                r#"class A { void f() { int a = x.y[0].z; } }"#,
                r#"(CompilationUnit (ClassDecl (Modifiers) (ClassBody (MethodDecl (Modifiers) (VoidType) (FormalParameters) (Block (LocalVariableDecl (Modifiers) (PrimitiveType) (VariableDeclarator (FieldAccess (ArrayAccess (FieldAccess (Name)) (Literal))))))))))"#,
            ),
        ];
        for (source, expected) in cases {
            let tree = parse(source).map(|unit| unit.tree);

            let shown = tree.map(|t| {
                let text: String = t
                    .tokens()
                    .iter()
                    .map(|token| &source[token.start..token.end])
                    .collect();
                (t.to_string(), text == source)
            });
            assert_eq!(shown, Ok((expected.to_owned(), true)), "{source}");
        }
    }

    #[test]
    fn when_parse_with_invalid_input_then_returns_an_error_at_its_offset() {
        let cases = [
            (r#"class A { void f() { List<A>= b; } }"#, 21),
            (r#"class A { void f() { bar; } }"#, 21),
            (r#"class A { void f() { int; } }"#, 21),
            (r#"class A { void f() { x -> 1; } }"#, 21),
            (
                r#"class A { void f() { switch (x) { case A a when a.b() -> 1; } } }"#,
                57,
            ),
            (r#"class A { void f() { for (i; ; j) {} } }"#, 26),
            (r#"class A { void f() { for (;; j) {} } }"#, 29),
            (r#"class A { void f() { (a); } }"#, 21),
            (r#"class A { void f() { -a; } }"#, 21),
            (r#"class A { /* open"#, 10),
            (r#"class A { void g() { <T>f(); } }"#, 24),
            (
                r#"class A {}
import a.B;"#,
                11,
            ),
            (r#"class A {"#, 9),
            (r#"class A { void f() {"#, 20),
            (r#"enum E { A"#, 10),
            (r#"class A { int x = 1 }"#, 19),
            (r#"module m { bad x; }"#, 11),
            (r#"module m { provides A B; }"#, 22),
            (r#"class A { void f() { case 1: } }"#, 21),
            (r#"class A { void f() { else {} } }"#, 21),
            (r#"class A { void f() { x = ; } }"#, 25),
            (r#"class A { void f() { switch (x) { case 1 x(); } } }"#, 41),
            (r#"class A { void f() { switch (x) { foo; } } }"#, 34),
            (r#"class A { void f() { switch (x) { case 1: f(); "#, 46),
            (r#"class 1 {}"#, 5),
            ("class A {\n  int /* c\n  */ ;\n}", 15),
            ("class A { void f() { a.<T\n>>b(); } }", 27),
            (
                "class A {\n  boolean f() {\n    return o instanceof R(int\n    );\n  }\n}",
                55,
            ),
            (
                "class A {\n  boolean f() {\n    return o instanceof R(S(int\n    ));\n  }\n}",
                57,
            ),
            (
                "class A {\n  boolean f() {\n    return o instanceof R(int a, int // c\n    );\n  }\n}",
                62,
            ),
            ("enum E {\n  A\n  1\n}", 12),
            ("class A { void f() { try {} catch (E\n) {} } }", 36),
            ("enum E { @X\n1 }", 11),
            ("enum E {\n1 }", 9),
            ("class A { void f() { a.\n; } }", 23),
            ("import\n;", 6),
            ("class A { void\n() {} }", 14),
            ("package\n;", 7),
            ("class A { void f(int\n) {} }", 20),
            ("record R(int\n) {}", 12),
            ("class A { void f() { g(a::\n); } }", 26),
            ("class\n{}", 5),
            ("class A<\n> {}", 8),
            (r#"@ int x;"#, 1),
            (r#"class A<T> { List<T x; }"#, 20),
            (r#"class A { int x = <T>; }"#, 21),
            (r#"class A { void f() { a = new int[1][ ; } }"#, 37),
            (r#"import a.;"#, 8),
            (r#"class A { void f() { g(1, ; } }"#, 26),
            (r#"class A { sealed x; non-sealed y; }"#, 23),
            (r#"class A { void f() { default: ; } }"#, 21),
        ];
        for (source, offset) in cases {
            let error = parse(source).map_err(|e| e.offset);

            assert_eq!(error.map(|_| ()), Err(offset), "{source}");
        }
    }
}
