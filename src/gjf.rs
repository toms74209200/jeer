pub mod builder;
pub mod cleanups;
pub mod comments;
pub mod doc;
pub mod imports;
pub mod input;
pub mod output;
pub mod visitor;

use builder::{Builder, Unformatted};
use input::{CommentKind, TokKind};
use output::{JavaOutput, Pending};
use visitor::Visitor;

use crate::lexer::token::TokenClass;
use crate::parser::syntax::{Element, Node};
use crate::parser::{self, Unit};
use crate::pipeline::{Position, Rejection};
use crate::steps;

fn significant(node: &Node) -> Node {
    let mut children = Vec::new();
    for child in &node.children {
        match child {
            Element::Token(token) => match token.kind.class() {
                TokenClass::Significant => children.push(Element::Token(*token)),
                TokenClass::Trivia => {}
            },
            Element::Node(child) => children.push(Element::Node(significant(child))),
        }
    }
    Node {
        kind: node.kind,
        children,
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Options {}

pub fn step(options: Options) -> impl Fn(&str) -> Result<String, Rejection> {
    move |text| {
        let Options {} = options;
        let unit = parser::parse(text).map_err(|e| Rejection {
            position: Position::Offset(e.offset),
            message: e.message,
        })?;
        match format(text, &unit) {
            Ok(formatted) => {
                steps::unused_imports::apply(&formatted).and_then(|used| imports::reorder(&used))
            }
            Err(Unformatted::Deferred) => Ok(text.to_owned()),
            Err(Unformatted::Rejected { offset, message }) => Err(Rejection {
                position: Position::Offset(offset),
                message,
            }),
            Err(Unformatted::RejectedAt {
                line,
                column,
                message,
            }) => Err(Rejection {
                position: Position::LineColumn { line, column },
                message,
            }),
        }
    }
}

fn format(source: &str, unit: &Unit) -> Result<String, Unformatted> {
    let cleaned = cleanups::reorder_modifiers(&cleanups::ast_cleanups(source, unit))?;
    match cleaned == source {
        true => format_cleaned(source, unit),
        false => {
            let in_cleaned = |offset: usize, message: String| {
                let before = &cleaned[..offset.min(cleaned.len())];
                Unformatted::RejectedAt {
                    line: before.matches('\n').count() + 1,
                    column: before
                        .rsplit('\n')
                        .next()
                        .map_or(0, |line| line.chars().count())
                        + 1,
                    message,
                }
            };
            crate::parser::parse(&cleaned)
                .map_err(|error| in_cleaned(error.offset, error.message))
                .and_then(|reparsed| {
                    format_cleaned(&cleaned, &reparsed).map_err(|error| match error {
                        Unformatted::Rejected { offset, message } => in_cleaned(offset, message),
                        Unformatted::Deferred | Unformatted::RejectedAt { .. } => error,
                    })
                })
        }
    }
}

fn format_cleaned(source: &str, unit: &Unit) -> Result<String, Unformatted> {
    let input = input::build(source, &unit.tree.tokens());
    let javadoc = input.tokens.iter().any(|token| {
        token.before.iter().chain(&token.after).any(|tok| {
            let markdown = tok.text.starts_with("///") && !tok.text.starts_with("////");
            markdown || matches!(tok.kind, TokKind::Comment(CommentKind::Doc, _))
        })
    });
    if javadoc {
        return Err(Unformatted::Deferred);
    }
    let tree = significant(&unit.tree);
    let mut visitor = Visitor {
        builder: Builder::from(&input),
        source,
        decoded: &unit.text,
    };
    visitor.compilation_unit(&tree)?;
    let (doc, blank_lines) = visitor.builder.build()?;
    let layout = doc::solve(&doc, 100, &comments::rewrite);
    let mut output = JavaOutput {
        input_lines: input.line_ranges.clone(),
        blank_lines,
        taken: layout.taken.clone(),
        pending: Pending::default(),
    };
    layout.replay(&mut output);
    Ok(String::from(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn when_format_with_oracle_fixtures_then_returns_what_google_java_format_returns() {
        let cases = [
            (
                include_str!("../tests/fixtures/gjf/accept-implicit-class-field/input.java"),
                include_str!("../tests/fixtures/gjf/accept-implicit-class-field/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-between-members/input.java"),
                include_str!("../tests/fixtures/gjf/comment-between-members/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-block/input.java"),
                include_str!("../tests/fixtures/gjf/comment-block/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-block-indent/input.java"),
                include_str!("../tests/fixtures/gjf/comment-block-indent/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-end-of-file/input.java"),
                include_str!("../tests/fixtures/gjf/comment-end-of-file/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-header/input.java"),
                include_str!("../tests/fixtures/gjf/comment-header/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-in-arguments/input.java"),
                include_str!("../tests/fixtures/gjf/comment-in-arguments/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-in-empty-block/input.java"),
                include_str!("../tests/fixtures/gjf/comment-in-empty-block/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-in-expression/input.java"),
                include_str!("../tests/fixtures/gjf/comment-in-expression/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-javadoc-like/input.java"),
                include_str!("../tests/fixtures/gjf/comment-javadoc-like/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-line/input.java"),
                include_str!("../tests/fixtures/gjf/comment-line/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-long-line/input.java"),
                include_str!("../tests/fixtures/gjf/comment-long-line/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-positions/input.java"),
                include_str!("../tests/fixtures/gjf/comment-positions/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotation-arguments/input.java"),
                include_str!("../tests/fixtures/gjf/decl-annotation-arguments/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotation-arrays/input.java"),
                include_str!("../tests/fixtures/gjf/decl-annotation-arrays/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotation-members/input.java"),
                include_str!("../tests/fixtures/gjf/decl-annotation-members/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotation-type/input.java"),
                include_str!("../tests/fixtures/gjf/decl-annotation-type/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotations/input.java"),
                include_str!("../tests/fixtures/gjf/decl-annotations/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-blank-lines/input.java"),
                include_str!("../tests/fixtures/gjf/decl-blank-lines/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-class-modifiers/input.java"),
                include_str!("../tests/fixtures/gjf/decl-class-modifiers/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-constructor/input.java"),
                include_str!("../tests/fixtures/gjf/decl-constructor/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-enum/input.java"),
                include_str!("../tests/fixtures/gjf/decl-enum/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-enum-forms/input.java"),
                include_str!("../tests/fixtures/gjf/decl-enum-forms/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-field-forms/input.java"),
                include_str!("../tests/fixtures/gjf/decl-field-forms/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-fields/input.java"),
                include_str!("../tests/fixtures/gjf/decl-fields/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-generics/input.java"),
                include_str!("../tests/fixtures/gjf/decl-generics/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-interface/input.java"),
                include_str!("../tests/fixtures/gjf/decl-interface/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-long-signature/input.java"),
                include_str!("../tests/fixtures/gjf/decl-long-signature/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-method-forms/input.java"),
                include_str!("../tests/fixtures/gjf/decl-method-forms/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-methods/input.java"),
                include_str!("../tests/fixtures/gjf/decl-methods/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-nested-classes/input.java"),
                include_str!("../tests/fixtures/gjf/decl-nested-classes/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-package-annotations/input.java"),
                include_str!("../tests/fixtures/gjf/decl-package-annotations/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-type-clauses/input.java"),
                include_str!("../tests/fixtures/gjf/decl-type-clauses/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-assignment-ops/input.java"),
                include_str!("../tests/fixtures/gjf/expr-assignment-ops/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-binary-long/input.java"),
                include_str!("../tests/fixtures/gjf/expr-binary-long/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-dotted-chains/input.java"),
                include_str!("../tests/fixtures/gjf/expr-dotted-chains/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-flogger-chain/input.java"),
                include_str!("../tests/fixtures/gjf/expr-flogger-chain/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-format-escapes/input.java"),
                include_str!("../tests/fixtures/gjf/expr-format-escapes/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-long-arguments/input.java"),
                include_str!("../tests/fixtures/gjf/expr-long-arguments/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-nested-calls/input.java"),
                include_str!("../tests/fixtures/gjf/expr-nested-calls/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-operator-precedence/input.java"),
                include_str!("../tests/fixtures/gjf/expr-operator-precedence/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-tabular-arguments/input.java"),
                include_str!("../tests/fixtures/gjf/expr-tabular-arguments/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-unary/input.java"),
                include_str!("../tests/fixtures/gjf/expr-unary/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-implicit-class/input.java"),
                include_str!("../tests/fixtures/gjf/modern-implicit-class/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-sealed/input.java"),
                include_str!("../tests/fixtures/gjf/modern-sealed/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-local-vars/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-local-vars/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-return-long/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-return-long/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-annotation-value/input.java"),
                include_str!("../tests/fixtures/gjf/string-annotation-value/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-char-literals/input.java"),
                include_str!("../tests/fixtures/gjf/string-char-literals/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-concat-in-args/input.java"),
                include_str!("../tests/fixtures/gjf/string-concat-in-args/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-concat-long/input.java"),
                include_str!("../tests/fixtures/gjf/string-concat-long/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-empty/input.java"),
                include_str!("../tests/fixtures/gjf/string-empty/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-escapes/input.java"),
                include_str!("../tests/fixtures/gjf/string-escapes/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-in-call-long/input.java"),
                include_str!("../tests/fixtures/gjf/string-in-call-long/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-long-literal/input.java"),
                include_str!("../tests/fixtures/gjf/string-long-literal/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-long-no-spaces/input.java"),
                include_str!("../tests/fixtures/gjf/string-long-no-spaces/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/decl-annotation-between-modifiers/input.java"),
                include_str!(
                    "../tests/fixtures/gjf/decl-annotation-between-modifiers/expected.java"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-tabular-split-operators/input.java"),
                include_str!("../tests/fixtures/gjf/expr-tabular-split-operators/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-escaped-quote/input.java"),
                include_str!("../tests/fixtures/gjf/string-escaped-quote/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-type-prefix-supplementary/input.java"),
                include_str!("../tests/fixtures/gjf/expr-type-prefix-supplementary/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-annotation-parens/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-annotation-parens/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-empty-statements/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-empty-statements/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-member-semicolons/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-member-semicolons/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-modifier-order/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-modifier-order/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-top-level-semicolons/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-top-level-semicolons/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-escaped-modifier/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-escaped-modifier/expected.java"),
            ),
        ];
        for (input, expected) in cases {
            let unit = parser::parse(input).ok();

            let formatted = unit.map(|unit| format(input, &unit));

            assert_eq!(formatted, Some(Ok(expected.to_owned())), "{input}");
        }
    }

    #[test]
    fn when_format_with_oracle_rejected_fixtures_then_returns_the_oracle_message() {
        let cases = [
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-char-literal/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-char-literal/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-whitespace/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-whitespace/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-comment/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-comment/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-start/input.java"),
                include_str!(
                    "../tests/fixtures/gjf/reject-escaped-identifier-start/expected.error"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-part/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-part/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-keyword/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-keyword/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-operator/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-operator/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/ident-supplementary/input.java"),
                include_str!("../tests/fixtures/gjf/ident-supplementary/expected.error"),
            ),
            (
                include_str!(
                    "../tests/fixtures/gjf/reject-cleanup-supplementary-separator/input.java"
                ),
                include_str!(
                    "../tests/fixtures/gjf/reject-cleanup-supplementary-separator/expected.error"
                ),
            ),
        ];
        for (input, expected) in cases {
            let unit = parser::parse(input).ok();

            let message = match unit.map(|unit| format(input, &unit)) {
                Some(Err(
                    Unformatted::Rejected { message, .. } | Unformatted::RejectedAt { message, .. },
                )) => message,
                Some(Ok(_) | Err(Unformatted::Deferred)) | None => String::new(),
            };

            assert!(
                !message.is_empty() && expected.contains(&message),
                "{input}: {message}"
            );
        }
    }

    #[test]
    fn when_format_with_constructs_of_later_tasks_then_returns_deferred() {
        let cases = [
            "/** doc */\nclass A {}\n",
            "/// doc\nclass A {}\n",
            "record R(int x) {}\n",
            "class A { Runnable r = () -> {}; }\n",
            "class A { String s = \"\"\"\n  x\"\"\"; }\n",
            "class A { void f() { if (x) {} } }\n",
            "class A { Object o = new Object(); }\n",
            "class A { void f(A this) {} }\n",
            "class A { int @B [] x; }\n",
            "class A { @B int f() { return 0; } <T> @B T g() { return null; } }\n",
            "module m {}\n",
        ];
        for input in cases {
            let unit = parser::parse(input).ok();

            let formatted = unit.map(|unit| format(input, &unit));

            assert_eq!(formatted, Some(Err(Unformatted::Deferred)), "{input}");
        }
    }

    #[test]
    fn when_gjf_pipeline_with_oracle_rejected_fixtures_then_returns_the_oracle_line() {
        let cases = [
            (
                include_str!("../tests/fixtures/gjf/ident-illegal-arrow/input.java"),
                include_str!("../tests/fixtures/gjf/ident-illegal-arrow/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/ident-illegal-bullet/input.java"),
                include_str!("../tests/fixtures/gjf/ident-illegal-bullet/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/ident-illegal-ideographic-space/input.java"),
                include_str!(
                    "../tests/fixtures/gjf/ident-illegal-ideographic-space/expected.error"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/ident-supplementary/input.java"),
                include_str!("../tests/fixtures/gjf/ident-supplementary/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-bad-annotation/input.java"),
                include_str!("../tests/fixtures/gjf/reject-bad-annotation/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-broken-lambda/input.java"),
                include_str!("../tests/fixtures/gjf/reject-broken-lambda/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-case-without-switch/input.java"),
                include_str!("../tests/fixtures/gjf/reject-case-without-switch/expected.error"),
            ),
            (
                include_str!(
                    "../tests/fixtures/gjf/reject-cleanup-supplementary-separator/input.java"
                ),
                include_str!(
                    "../tests/fixtures/gjf/reject-cleanup-supplementary-separator/expected.error"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-else-without-if/input.java"),
                include_str!("../tests/fixtures/gjf/reject-else-without-if/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-char-literal/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-char-literal/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-comment/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-comment/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-part/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-part/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-identifier-start/input.java"),
                include_str!(
                    "../tests/fixtures/gjf/reject-escaped-identifier-start/expected.error"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-keyword/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-keyword/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-operator/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-operator/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-escaped-whitespace/input.java"),
                include_str!("../tests/fixtures/gjf/reject-escaped-whitespace/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-extra-brace/input.java"),
                include_str!("../tests/fixtures/gjf/reject-extra-brace/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-illegal-escape/input.java"),
                include_str!("../tests/fixtures/gjf/reject-illegal-escape/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-malformed-generic/input.java"),
                include_str!("../tests/fixtures/gjf/reject-malformed-generic/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-missing-paren/input.java"),
                include_str!("../tests/fixtures/gjf/reject-missing-paren/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-position-after-cleanup/input.java"),
                include_str!("../tests/fixtures/gjf/reject-position-after-cleanup/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-position-later-line/input.java"),
                include_str!("../tests/fixtures/gjf/reject-position-later-line/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-unclosed-comment/input.java"),
                include_str!("../tests/fixtures/gjf/reject-unclosed-comment/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-unterminated-text-block/input.java"),
                include_str!("../tests/fixtures/gjf/reject-unterminated-text-block/expected.error"),
            ),
        ];
        for (input, error) in cases {
            let line =
                crate::pipeline::Profile::Gjf.pipeline()(input).map_err(|rejection| rejection.line);
            let expected = error
                .strip_prefix('L')
                .and_then(|rest| rest.split(' ').next())
                .and_then(|line| line.parse::<usize>().ok());

            assert_eq!(line.err(), expected, "{input}");
        }
    }
}
