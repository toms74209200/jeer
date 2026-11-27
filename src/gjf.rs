pub mod builder;
pub mod cleanups;
pub mod comments;
pub mod doc;
pub mod imports;
pub mod input;
pub mod javadoc;
pub mod output;
pub mod reflow;
pub mod visitor;

use std::collections::HashMap;

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
pub struct Options {
    pub reflow_long_strings: bool,
}

pub fn step(options: Options) -> impl Fn(&str) -> Result<String, Rejection> {
    move |text| {
        let Options {
            reflow_long_strings,
        } = options;
        let unit = parser::parse(text).map_err(|e| Rejection {
            position: Position::Offset(e.offset),
            message: e.message,
        })?;
        match format(text, &unit) {
            Ok(formatted) => {
                let ordered = steps::unused_imports::apply(&formatted)
                    .and_then(|used| imports::reorder(&used))?;
                match reflow_long_strings {
                    true => settle(text, reflow_strings(&ordered)),
                    false => Ok(ordered),
                }
            }
            unformatted => settle(text, unformatted),
        }
    }
}

fn settle(text: &str, result: Result<String, Unformatted>) -> Result<String, Rejection> {
    match result {
        Ok(formatted) => Ok(formatted),
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

fn reflow_strings(text: &str) -> Result<String, Unformatted> {
    let too_long = text.lines().any(|line| line.encode_utf16().count() > 100);
    if !too_long {
        return Ok(text.to_owned());
    }
    let unit = parser::parse(text).map_err(|_| Unformatted::Deferred)?;
    let wrapped = reflow::wrap(text, &significant(&unit.tree));
    if wrapped == text {
        return Ok(wrapped);
    }
    let reformatted = format(text, &unit)?;
    if reformatted != text {
        return Err(Unformatted::Deferred);
    }
    parser::parse(&wrapped).map_err(|_| Unformatted::Deferred)?;
    Ok(wrapped)
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
    let comments = input
        .tokens
        .iter()
        .flat_map(|token| token.before.iter().chain(&token.after));
    let javadocs: HashMap<&str, Vec<javadoc::lexer::Token>> = comments
        .clone()
        .filter(|tok| matches!(tok.kind, TokKind::Comment(CommentKind::Doc, _)))
        .filter_map(|tok| {
            let text = javadoc::text::classic_comment_text(&tok.text);
            javadoc::lexer::lex(&text, javadoc::lexer::Flavor::Classic)
                .map(|tokens| (tok.text.as_str(), tokens))
        })
        .collect();
    let tree = significant(&unit.tree);
    let mut visitor = Visitor {
        builder: Builder::from(&input),
        source,
        decoded: &unit.text,
        hug_points: std::collections::HashMap::new(),
        not_huggable: std::collections::HashSet::new(),
        type_annotations: std::collections::HashSet::new(),
        markdown_positions: std::collections::HashMap::new(),
    };
    visitor.compilation_unit(&tree)?;
    let (doc, blank_lines) = visitor.builder.build()?;
    let markdown = visitor.markdown_positions;
    let rewrite = |text: &str, position: usize, max_width: usize, column: usize| {
        let formatted = javadocs.get(text).map(|tokens| {
            javadoc::writer::render(tokens, javadoc::lexer::Flavor::Classic, column, max_width)
        });
        match markdown.get(&position) {
            Some(visitor::Marked::Javadoc {
                text: comment,
                positions,
            }) => {
                let flavor = javadoc::lexer::Flavor::Markdown(positions);
                javadoc::lexer::lex(comment, flavor).map_or_else(
                    || text.to_owned(),
                    |tokens| javadoc::writer::render(&tokens, flavor, column, max_width),
                )
            }
            Some(visitor::Marked::Unsupported) => text.to_owned(),
            Some(visitor::Marked::LineComment) => text
                .split('\n')
                .map(|line| {
                    line.trim_matches(char::is_whitespace)
                        .trim_matches(|c: char| c <= ' ')
                })
                .collect::<Vec<_>>()
                .join(&format!("\n{}", " ".repeat(column))),
            None => comments::rewrite(formatted.as_deref().unwrap_or(text), max_width, column),
        }
    };
    let layout = doc::solve(&doc, 100, &rewrite);
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
                include_str!("../tests/fixtures/gjf/stmt-if-else/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-if-else/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-empty-blocks/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-empty-blocks/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-before-else/input.java"),
                include_str!("../tests/fixtures/gjf/comment-before-else/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-loops/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-loops/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-labels/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-labels/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-try/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-try/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-switch-old/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-switch-old/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/comment-in-switch/input.java"),
                include_str!("../tests/fixtures/gjf/comment-in-switch/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-switch-case/input.java"),
                include_str!("../tests/fixtures/gjf/string-switch-case/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-synchronized/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-synchronized/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-assert-plain/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-assert-plain/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-synchronized-wrap/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-synchronized-wrap/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-local-class-plain/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-local-class-plain/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-lambda/input.java"),
                include_str!("../tests/fixtures/gjf/expr-lambda/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-method-ref/input.java"),
                include_str!("../tests/fixtures/gjf/expr-method-ref/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-lambda-parens/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-lambda-parens/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/cleanup-enum-constant-arguments/input.java"),
                include_str!("../tests/fixtures/gjf/cleanup-enum-constant-arguments/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-lambda-hug-call/input.java"),
                include_str!("../tests/fixtures/gjf/expr-lambda-hug-call/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-lambda-hug-chain/input.java"),
                include_str!("../tests/fixtures/gjf/expr-lambda-hug-chain/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-lambda-params/input.java"),
                include_str!("../tests/fixtures/gjf/expr-lambda-params/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-new-array/input.java"),
                include_str!("../tests/fixtures/gjf/expr-new-array/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-switch-arrow/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-switch-arrow/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-assert-throw/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-assert-throw/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-new-class/input.java"),
                include_str!("../tests/fixtures/gjf/expr-new-class/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-casts/input.java"),
                include_str!("../tests/fixtures/gjf/expr-casts/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-instanceof-type/input.java"),
                include_str!("../tests/fixtures/gjf/expr-instanceof-type/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-ternary/input.java"),
                include_str!("../tests/fixtures/gjf/expr-ternary/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-method-chain/input.java"),
                include_str!("../tests/fixtures/gjf/expr-method-chain/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-anonymous-chain-head/input.java"),
                include_str!("../tests/fixtures/gjf/expr-anonymous-chain-head/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-simple/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-simple/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-empty/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-empty/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-one-line-tag/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-one-line-tag/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-field/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-field/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-paragraphs/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-paragraphs/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-blank-lines/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-blank-lines/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-params/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-params/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-wrap/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-wrap/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-lists/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-lists/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-pre/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-pre/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-code/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-code/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-html-entities/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-html-entities/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/imports-javadoc-use/input.java"),
                include_str!("../tests/fixtures/gjf/imports-javadoc-use/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-bare/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-bare/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-short/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-short/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-text/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-text/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-paragraphs/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-paragraphs/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-heading/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-heading/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-setext-heading/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-setext-heading/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-thematic-break/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-thematic-break/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-indented-code/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-indented-code/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-link-definition/input.java"),
                include_str!(
                    "../tests/fixtures/gjf/javadoc-markdown-link-definition/expected.java"
                ),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-code-span/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-code-span/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-lists/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-lists/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-blockquote/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-blockquote/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-fenced-code/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-fenced-code/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-markdown-table/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-markdown-table/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-literal-lt/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-literal-lt/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-snippet/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-snippet/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-table/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-table/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-blockquote/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-blockquote/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-headers/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-headers/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-br/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-br/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-html-comment/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-html-comment/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-link/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-link/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/javadoc-moe-strip/input.java"),
                include_str!("../tests/fixtures/gjf/javadoc-moe-strip/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-jspecify-method/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-jspecify-method/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-checker-method/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-checker-method/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-field/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-field/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-local/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-local/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-parameter/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-parameter/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-type-parameters/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-type-parameters/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/type-use-other-package/input.java"),
                include_str!("../tests/fixtures/gjf/type-use-other-package/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-text-block/input.java"),
                include_str!("../tests/fixtures/gjf/string-text-block/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/string-text-block-in-call/input.java"),
                include_str!("../tests/fixtures/gjf/string-text-block-in-call/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-text-block-indent/input.java"),
                include_str!("../tests/fixtures/gjf/modern-text-block-indent/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-record/input.java"),
                include_str!("../tests/fixtures/gjf/modern-record/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-generic-record/input.java"),
                include_str!("../tests/fixtures/gjf/modern-generic-record/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/stmt-local-class/input.java"),
                include_str!("../tests/fixtures/gjf/stmt-local-class/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-switch-expression/input.java"),
                include_str!("../tests/fixtures/gjf/modern-switch-expression/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-yield/input.java"),
                include_str!("../tests/fixtures/gjf/modern-yield/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-record-pattern/input.java"),
                include_str!("../tests/fixtures/gjf/modern-record-pattern/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-enhanced-for-pattern/input.java"),
                include_str!("../tests/fixtures/gjf/modern-enhanced-for-pattern/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-unnamed/input.java"),
                include_str!("../tests/fixtures/gjf/modern-unnamed/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-var/input.java"),
                include_str!("../tests/fixtures/gjf/modern-var/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/expr-instanceof/input.java"),
                include_str!("../tests/fixtures/gjf/expr-instanceof/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf/modern-module/input.java"),
                include_str!("../tests/fixtures/gjf/modern-module/expected.java"),
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
    fn when_format_with_javadoc_text_then_returns_the_oracle_output() {
        let input = "class A {\n  /**\n   * <p>Leading paragraph with a well-\n   * known word, a dash-\n   * and pair, plus foo @bar here, x\u{b2}-\n   * foo, \u{1d400}-\n   * bar, \u{e9}-\n   * tude.\n   *\n   * Inferred paragraph.</p>\n   * <p class=\"x\">Classed.\n   *\n   * @param <T> the type parameter whose description is long enough that it has to wrap onto another line\n   * @param a the a\n   *\n   * still a\n   */\n  <T> void f(int a) {}\n\n  /** @hide */\n  int w;\n\n  /** a\u{2028}b */\n  int v;\n\n  int x; /** trailing */\n\n  int y, /** b */ z;\n\n  void g(int a) {\n    g(a, /** b */ 1);\n  }\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class A {\n  /**\n   * Leading paragraph with a well-known word, a dash- and pair, plus foo @bar here, x\u{b2}- foo, \u{1d400}-\n   * bar, \u{e9}-tude.\n   *\n   * <p>Inferred paragraph.\n   *\n   * <p class=\"x\">Classed.\n   *\n   * @param <T> the type parameter whose description is long enough that it has to wrap onto another\n   *     line\n   * @param a the a\n   *     <p>still a\n   */\n  <T> void f(int a) {}\n\n  /** @hide */\n  int w;\n\n  /**\n   * a\u{2028}b\n   */\n  int v;\n\n  int x;\n\n  /** trailing */\n  int y,\n      /** b */\n      z;\n\n  void g(int a) {\n    g(\n        a,\n        /** b */\n        1);\n  }\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_javadoc_markup_then_returns_the_oracle_output() {
        let input = "class B {\n  /**\n   * Outer:\n   * <ul>\n   * <li>first item text that is long enough to wrap past the line limit of one hundred columns here\n   * <li>nested:\n   *   <ol>\n   *   <li>inner\n   *   </ol>\n   * </li>\n   * </ul>\n   * After {@link Map#get(Object) the {nested} braces} and <code>x  y\n   * z</code> plus { lone } and {@code a}.\n   * See {@link a {@snippet b} c} and a break<br clear=\"all\">here.\n   *\n   * <pre>{@code\n   *     int a = 1;\n   *       int b;\n   * }</pre>\n   *\n   * <pre>\n   * {@code\n   *   tail();}\n   * </pre>\n   *\n   * <pre>{@code}</pre>\n   *\n   * <pre>{@code\n   *   x\n   * } more\n   * </pre>\n   *\n   * <pre>\n   *   plain <code>c</code> d\n   * </pre>\n   *\n   * @param a uses <ul><li>one</ul>\n   */\n  void f(int a) {}\n\n  /** Unbalanced <pre> then @return x */\n  int g() { return 0; }\n\n  /**\n   *   Open {@code\n   * @return x}\n   */\n  int h() { return 0; }\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class B {\n  /**\n   * Outer:\n   *\n   * <ul>\n   *   <li>first item text that is long enough to wrap past the line limit of one hundred columns\n   *       here\n   *   <li>nested:\n   *       <ol>\n   *         <li>inner\n   *       </ol>\n   * </ul>\n   *\n   * After {@link Map#get(Object) the {nested} braces} and <code>x  y\n   * z</code> plus { lone } and {@code a}. See {@link a {@snippet b} c} and a break<br clear=\"all\">\n   * here.\n   *\n   * <pre>{@code\n   * int a = 1;\n   *   int b;\n   * }</pre>\n   *\n   * <pre>{@code\n   * tail();\n   * }</pre>\n   *\n   * <pre>{@code}</pre>\n   *\n   * <pre>{@code\n   *   x\n   * } more\n   * </pre>\n   *\n   * <pre>\n   *   plain <code>c</code> d\n   * </pre>\n   *\n   * @param a uses\n   *     <ul>\n   *       <li>one\n   *     </ul>\n   */\n  void f(int a) {}\n\n  /** Unbalanced <pre> then @return x */\n  int g() {\n    return 0;\n  }\n\n  /**\n   *   Open {@code\n   * @return x}\n   */\n  int h() {\n    return 0;\n  }\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_slash_slash_slash_comments_then_returns_the_oracle_output() {
        let input = "class A {\n  void f() {\n    g(); ///\n    g(); //w\n    g(); ///ab\n    g(); ////ab\n    ///one\n    ///two\n    g();\n///three\n///four\n    g();\n    ///x\n    int a = 1;\n    /// local doc\n    int b = 2;\n  }\n\n  ///y\n  int c, d;\n\n  /// many doc\n  int e, f;\n\n  int x; ///w\n  int y;\n\n  ///x\n  int z;\n\n  ////long four slashes at a declaration\n  void h() {}\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class A {\n  void f() {\n    g(); ///\n    g(); // w\n    g();\n    /// ab\n    g(); //// ab\n         /// one\n         /// two\n    g();\n    /// three\n    /// four\n    g();\n    /// x\n    int a = 1;\n    /// local doc\n    int b = 2;\n  }\n\n  /// y\n  int c, d;\n\n  /// many doc\n  int e, f;\n\n  int x; // /w\n  int y;\n\n  ///x\n  int z;\n\n  ////long four slashes at a declaration\n  void h() {}\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_type_use_annotations_then_returns_the_oracle_output() {
        let input = "import java.util.Map;\nimport org.jspecify.annotations.NonNull;\nimport org.jspecify.annotations.Nullable;\n\n@Deprecated @Nullable class B {\n  @Nullable @NonNull Object f() { return null; }\n  @Deprecated @Nullable <T> T g() { return null; }\n  @Nullable Object a, b;\n  void p(@Nullable Map<String, Map<String, Map<String, Map<String, String>>>> someLongParameterName) {}\n  @Deprecated @Nullable B() {}\n  @Deprecated @Nullable enum E { X }\n  @Deprecated @Nullable @interface I {}\n  void h() { try { h(); } catch (@Nullable RuntimeException | Error e) {} }\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "import java.util.Map;\nimport org.jspecify.annotations.NonNull;\nimport org.jspecify.annotations.Nullable;\n\n@Deprecated\n@Nullable\nclass B {\n  @Nullable @NonNull Object f() {\n    return null;\n  }\n\n  @Deprecated\n  @Nullable <T> T g() {\n    return null;\n  }\n\n  @Nullable Object a, b;\n\n  void p(\n      @Nullable Map<String, Map<String, Map<String, Map<String, String>>>> someLongParameterName) {}\n\n  @Deprecated\n  @Nullable\n  B() {}\n\n  @Deprecated\n  @Nullable\n  enum E {\n    X\n  }\n\n  @Deprecated\n  @Nullable\n  @interface I {}\n\n  void h() {\n    try {\n      h();\n    } catch (@Nullable RuntimeException | Error e) {\n    }\n  }\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_text_blocks_then_returns_the_oracle_output() {
        let input = "class A {\n  String a = \"\"\"\n  x\"\"\";\n  String b = \"\"\"\n    one\n\n    two   \n\"\"\";\n  String c = \"\"\"   \n      deep\n    \"\"\";\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class A {\n  String a =\n      \"\"\"\n      x\\\n      \"\"\";\n  String b =\n\"\"\"\n    one\n\n    two\n\"\"\";\n  String c =\n      \"\"\"\n        deep\n      \"\"\";\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_text_block_whitespace_then_returns_the_oracle_output() {
        let input = "class A {\n  String s0 = \"\"\"\n\u{85}    a\u{85}\n    b\n    \"\"\";\n  String s1 = \"\"\"\n\u{a0}    a\u{a0}\n    b\n    \"\"\";\n  String s2 = \"\"\"\n\u{2007}    a\u{2007}\n    b\n    \"\"\";\n  String s3 = \"\"\"\n\u{202f}    a\u{202f}\n    b\n    \"\"\";\n  String s4 = \"\"\"\n\u{1c}    a\u{1c}\n    b\n    \"\"\";\n  String s5 = \"\"\"\n\u{1f}    a\u{1f}\n    b\n    \"\"\";\n  String s6 = \"\"\"\n\u{3000}    a\u{3000}\n    b\n    \"\"\";\n  String s7 = \"\"\"\n\t    a\t\n    b\n    \"\"\";\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class A {\n  String s0 =\n\"\"\"\n\u{85}    a\u{85}\n    b\n\"\"\";\n  String s1 =\n\"\"\"\n\u{a0}    a\u{a0}\n    b\n\"\"\";\n  String s2 =\n\"\"\"\n\u{2007}    a\u{2007}\n    b\n\"\"\";\n  String s3 =\n\"\"\"\n\u{202f}    a\u{202f}\n    b\n\"\"\";\n  String s4 =\n      \"\"\"\n       a\n      b\n      \"\"\";\n  String s5 =\n      \"\"\"\n       a\n      b\n      \"\"\";\n  String s6 =\n      \"\"\"\n       a\n      b\n      \"\"\";\n  String s7 =\n      \"\"\"\n       a\n      b\n      \"\"\";\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_records_then_returns_the_oracle_output() {
        let input = "import java.io.Serializable;\nimport java.util.List;\nclass A {\n  record Empty() {}\n  record Box<T>(T value) {}\n  record Var(int first, String... rest) implements Comparable<Var>, Serializable {\n    Var {\n      first = first + 1;\n    }\n    public int compareTo(Var other) { return 0; }\n  }\n  record LongHeader(List<String> someVeryLongComponentName, List<String> anotherVeryLongComponentName) {}\n  @Deprecated record Annotated(@Deprecated int value) {}\n  record Gen<T extends Comparable<T>>(T value) implements Comparable<Gen<T>> {\n    public int compareTo(Gen<T> other) { return 0; }\n  }\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "import java.io.Serializable;\nimport java.util.List;\n\nclass A {\n  record Empty() {}\n\n  record Box<T>(T value) {}\n\n  record Var(int first, String... rest) implements Comparable<Var>, Serializable {\n    Var {\n      first = first + 1;\n    }\n\n    public int compareTo(Var other) {\n      return 0;\n    }\n  }\n\n  record LongHeader(\n      List<String> someVeryLongComponentName, List<String> anotherVeryLongComponentName) {}\n\n  @Deprecated\n  record Annotated(@Deprecated int value) {}\n\n  record Gen<T extends Comparable<T>>(T value) implements Comparable<Gen<T>> {\n    public int compareTo(Gen<T> other) {\n      return 0;\n    }\n  }\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_switch_expressions_then_returns_the_oracle_output() {
        let input = "class A {\n  int f(Object o) {\n    if (o instanceof final String s) {}\n    if (o instanceof SomeVeryLongRecordName(SomeOtherLongRecordName(var firstComponent, var secondComponent), _)) {}\n    g(switch (o) { case Integer i -> i; default -> 0; }, 1);\n    return switch (o) {\n      case Integer i, Long l when o.hashCode() > 0 -> 1;\n      case String s -> {\n        yield s.length() + 1;\n      }\n      default -> throw new IllegalStateException();\n    };\n  }\n}\n";
        let unit = parser::parse(input).ok();

        let formatted = unit.map(|unit| format(input, &unit));

        let expected = "class A {\n  int f(Object o) {\n    if (o instanceof final String s) {}\n    if (o\n        instanceof\n        SomeVeryLongRecordName(\n            SomeOtherLongRecordName(var firstComponent, var secondComponent),\n            _)) {}\n    g(\n        switch (o) {\n          case Integer i -> i;\n          default -> 0;\n        },\n        1);\n    return switch (o) {\n      case Integer i, Long l when o.hashCode() > 0 -> 1;\n      case String s -> {\n        yield s.length() + 1;\n      }\n      default -> throw new IllegalStateException();\n    };\n  }\n}\n";
        assert_eq!(formatted, Some(Ok(expected.to_owned())));
    }

    #[test]
    fn when_format_with_modules_then_returns_the_oracle_output() {
        let cases = [
            (
                "import com.example.Ann;\n\n/** The app. */\n@Ann @Deprecated open module com.example.app {\n  requires static transitive java.sql;\n  requires java.base;\n  exports com.example.api to com.example.one, com.example.two;\n  provides com.example.Spi with com.example.A, com.example.B;\n  uses com.example.Spi;\n}\n",
                "import com.example.Ann;\n\n/** The app. */\n@Ann\n@Deprecated\nopen module com.example.app {\n  requires static transitive java.sql;\n  requires java.base;\n\n  exports com.example.api to\n      com.example.one,\n      com.example.two;\n\n  provides com.example.Spi with\n      com.example.A,\n      com.example.B;\n\n  uses com.example.Spi;\n}\n",
            ),
            ("module m {}\n", "module m {}\n"),
        ];
        for (input, expected) in cases {
            let unit = parser::parse(input).ok();

            let formatted = unit.map(|unit| format(input, &unit));

            assert_eq!(formatted, Some(Ok(expected.to_owned())), "{input}");
        }
    }

    #[test]
    fn when_step_with_reflow_on_reflow_fixtures_then_returns_what_google_java_format_returns() {
        let cases = [
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-argument/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-argument/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-assignment/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-assignment/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-return/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-return/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-concatenation/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-concatenation/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-annotation/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-annotation/expected.java"),
            ),
            (
                include_str!("../tests/fixtures/gjf-reflow/reflow-unchanged/input.java"),
                include_str!("../tests/fixtures/gjf-reflow/reflow-unchanged/expected.java"),
            ),
        ];
        let reflowing = step(Options {
            reflow_long_strings: true,
        });
        for (input, expected) in cases {
            let twice = reflowing(input).and_then(|once| reflowing(&once));
            let again = twice
                .as_deref()
                .map_or(None, |settled| reflowing(settled).ok());

            assert_eq!(twice, Ok(expected.to_owned()), "{input}");
            assert_eq!(again.as_deref(), Some(expected), "{input}");
        }
    }

    #[test]
    fn when_step_with_reflow_on_escapes_then_returns_the_oracle_output() {
        let input = "class A {\n  String a = prefix() + \"A string after a call, long enough that the concatenation passes the limit of one hundred columns.\";\n  String b = \"A first literal that is long enough on its own to pass the column limit of one hundred, \" + \"and more\";\n  String c = \"Line one of a message that has escapes\\nline two\\r\\nline three\\tand a tab, also a backslash \\\\ in it, long.\";\n  String f = \"A first literal that is long enough on its own to pass the column limit of one hundred columns, \" + \"and more\";\n  String g = \"The first line of a long message that has to be wrapped to stay inside the limit\\n\" + \" and a second literal that starts with a space, long enough to pass the limit.\";\n  String h = \" A literal that starts with a space and is long enough to pass the limit of one hundred columns.\";\n  String i = \"A literal of many words that is long enough to pass the limit of one hundred columns by a lot\" + \"Suffix\";\n  String j = \"A long message that ends with a newline escape and passes the limit of one hundred columns.\\n\";\n  String d = \"Two  spaces  between  words  in  a  string  that  is  long  enough  to  pass  the  column  limit  here.\";\n  int e = \"A receiver literal that is long enough to pass the column limit of one hundred columns by far\".length();\n}\n";
        let reflowing = step(Options {
            reflow_long_strings: true,
        });

        let twice = reflowing(input).and_then(|once| reflowing(&once));

        let expected = "class A {\n  String a =\n      prefix()\n          + \"A string after a call, long enough that the concatenation passes the limit of one\"\n          + \" hundred columns.\";\n  String b =\n      \"A first literal that is long enough on its own to pass the column limit of one hundred, \"\n          + \"and more\";\n  String c =\n      \"Line one of a message that has escapes\\n\"\n          + \"line two\\r\\n\"\n          + \"line three\\tand a tab, also a backslash \\\\ in it, long.\";\n  String f =\n      \"A first literal that is long enough on its own to pass the column limit of one hundred\"\n          + \" columns, and more\";\n  String g =\n      \"The first line of a long message that has to be wrapped to stay inside the limit\\n\"\n          + \" and a second literal that starts with a space, long enough to pass the limit.\";\n  String h =\n      \" A literal that starts with a space and is long enough to pass the limit of one hundred\"\n          + \" columns.\";\n  String i =\n      \"A literal of many words that is long enough to pass the limit of one hundred columns by a\"\n          + \" lotSuffix\";\n  String j =\n      \"A long message that ends with a newline escape and passes the limit of one hundred\"\n          + \" columns.\\n\";\n  String d =\n      \"Two  spaces  between  words  in  a  string  that  is  long  enough  to  pass  the  column \"\n          + \" limit  here.\";\n  int e =\n      \"A receiver literal that is long enough to pass the column limit of one hundred columns by far\"\n          .length();\n}\n";
        assert_eq!(twice, Ok(expected.to_owned()));
    }

    #[test]
    fn when_step_with_reflow_where_google_java_format_wraps_otherwise_then_returns_the_input() {
        let cases = [
            "class A {\n  String k = \"A literal long enough to pass the limit of one hundred columns, then followed by a text block\" + \"\"\"\n      tail\n      \"\"\";\n}\n",
            "class A {\n  void f() {\n    g(); ///w\n    log(\"This message is long enough that it does not fit on one line together with the call around it.\");\n  }\n}\n",
        ];
        let reflowing = step(Options {
            reflow_long_strings: true,
        });
        for input in cases {
            let formatted = reflowing(input);

            assert_eq!(formatted, Ok(input.to_owned()), "{input}");
        }
    }

    #[test]
    fn when_reflow_strings_with_unparsable_text_then_returns_deferred() {
        let input = format!("class A {{ int x = ; // {}\n", "y".repeat(100));

        let reflowed = reflow_strings(&input);

        assert_eq!(reflowed, Err(Unformatted::Deferred));
    }

    #[test]
    fn when_format_with_constructs_of_later_tasks_then_returns_deferred() {
        let cases = [
            "/// a <b>x</b>\nclass A {}\n",
            "class A { <T> @B int f() { return 0; } }\n",
            "class A { void f(A this) {} }\n",
            "class A { int @B [] x; }\n",
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
                include_str!("../tests/fixtures/gjf/reject-missing-semicolon/input.java"),
                include_str!("../tests/fixtures/gjf/reject-missing-semicolon/expected.error"),
            ),
            (
                include_str!("../tests/fixtures/gjf/reject-unbalanced-brace/input.java"),
                include_str!("../tests/fixtures/gjf/reject-unbalanced-brace/expected.error"),
            ),
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

    #[test]
    fn when_gjf_pipeline_with_an_escaped_parameter_then_returns_the_oracle_line() {
        let cases = [
            (
                "class A {\n  void m() {\n    try {\n    } catch (\n        \\u002f\\u002a c \\u002a\\u002f final Exception e) {}\n  }\n}\n",
                "L5 google-java-format(google-java-format) error: did not generate token \"\\\"",
            ),
            (
                "class A {\n  void m() {\n    try {\n    } catch (\n        \\u002f\\u002a c \\u002a\\u002f final A | B e) {}\n  }\n}\n",
                "L5 google-java-format(google-java-format) error: did not generate token \"\\\"",
            ),
            (
                "class A {\n  Object o =\n      (var a,\n          var \\u0062) -> a;\n}\n",
                "L3 google-java-format(google-java-format) error: expected token: '\\'; generated b instead",
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

    #[test]
    fn when_format_with_markdown_javadoc_then_returns_the_oracle_output() {
        let cases = [
            (
                "class A {\n  /// a|b\n  /// :-:\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a|b :-: rest text with enough words after it that the formatter has to wrap this line onto the\n  /// next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b |\n  /// |---||---|\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b | |---||---| rest text with enough words after it that the formatter has to wrap this\n  /// line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b |\n  /// |--- ---|\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b | |--- ---| rest text with enough words after it that the formatter has to wrap this\n  /// line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a |\n  /// |:|\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | |:| rest text with enough words after it that the formatter has to wrap this line onto\n  /// the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// a | b\n  /// -- --\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a | b\n  /// -- --\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// Text.\n  ///\n  /// > | a | b |\n  /// > |---|---|\n  void f() {}\n}\n",
                "class A {\n  /// Text.\n  ///\n  /// > | a | b |\n  /// > |---|---|\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Intro   text\n  /// more   text\n  /// | a | b |\n  /// |---|---|\n  /// | 1 | 2 |\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Intro text more text\n  ///\n  /// | a | b |\n  /// |---|---|\n  /// | 1 | 2 |\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b | c |\n  /// |:---|---:|:-:|\n  /// | 1 | 2 | 3 |\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b | c |\n  /// |:---|---:|:-:|\n  /// | 1 | 2 | 3 |\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// | 1 | 2 |\n  /// after\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// | 1 | 2 |\n  /// after\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// a \\| b | c\n  /// ---|---\n  /// x | y\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a \\| b | c\n  /// ---|---\n  /// x | y\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item\n  ///\n  ///   | a | b |\n  ///   |---|---|\n  ///   | 1 | 2 |\n  /// - next\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item\n  ///\n  ///   | a | b |\n  ///   |---|---|\n  ///   | 1 | 2 |\n  ///\n  /// - next\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > | a | b |\n  /// > |---|---|\n  /// > | 1 | 2 |\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >\n  /// >\n  /// > | a | b |\n  /// > |---|---|\n  /// > | 1 | 2 |\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a |\n  ///     |---|\n  /// | 1 |\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a |\n  ///     |---|\n  /// | 1 |\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a |\n  /// | -x- |\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | | -x- | rest text with enough words after it that the formatter has to wrap this line\n  /// onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// lazy   row\n  /// more\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// lazy   row\n  /// more\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// | xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx | y |\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b |\n  /// |---|---|\n  /// | xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx | y |\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// a\n  /// |---|\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a |---| rest text with enough words after it that the formatter has to wrap this line onto the\n  /// next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// a | b\n  /// --- | ---\n  /// 1 | 2\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a | b\n  /// --- | ---\n  /// 1 | 2\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a |\n  /// |---|\n  /// |\n  /// after   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a |\n  /// |---|\n  ///\n  /// | after text with enough words after it that the formatter has to wrap this line onto the next\n  /// one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a |\n  /// |---|\n  /// | 1 |\n  ///\n  /// paragraph   after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a |\n  /// |---|\n  /// | 1 |\n  ///\n  /// paragraph after with enough words after it that the formatter has to wrap this line onto the\n  /// next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// | a | b | c |\n  /// |---|---|\n  /// rest   text with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// | a | b | c | |---|---| rest text with enough words after it that the formatter has to wrap\n  /// this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// a | b | \n  /// ---|---|---\n  /// 1 | 2 | 3\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// a | b |\n  /// ---|---|---\n  /// 1 | 2 | 3\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```\n  /// x\n  ///\n  void f() {}\n}\n",
                "class A {\n  /// ```\n  /// x\n  /// ```\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// - a\n  ///\n  ///   ```\n  ///   x\n  ///\n  ///\n  void f() {}\n}\n",
                "class A {\n  /// - a\n  ///\n  ///   ```\n  ///   x\n  ///\n  ///   ```\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Text.\n  ///\n  /// > ```\n  /// > quoted\n  /// > ```\n  void f() {}\n}\n",
                "class A {\n  /// Text.\n  ///\n  /// > ```\n  /// > quoted\n  /// > ```\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// > ```\n  /// > quoted\n  /// > ```\n  void f() {}\n}\n",
                "class A {\n  /// >\n  /// >\n  /// > ```\n  /// > quoted\n  /// > ```\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Text with enough words after it that the formatter has to wrap this line onto the next one\n  /// ```\n  /// code\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Text with enough words after it that the formatter has to wrap this line onto the next one\n  ///\n  /// ```\n  /// code\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```\n  /// a\n  ///\n  /// b\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```\n  /// a\n  ///\n  /// b\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item\n  ///\n  ///   ```\n  ///   code\n  ///   ```\n  /// - next\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item\n  ///\n  ///   ```\n  ///   code\n  ///   ```\n  ///\n  /// - next\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > ```\n  /// > quoted   code\n  /// > ```\n  /// > after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >\n  /// >\n  /// > ```\n  /// > quoted   code\n  /// > ```\n  /// >\n  /// > after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  ///   ```\n  ///   a\n  ///     b\n  ///  c\n  ///   ```\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```\n  /// a\n  ///   b\n  /// c\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```  info\\_with\\*escapes  \n  /// x\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```info_with*escapes\n  /// x\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```\n  /// x\n  /// `````\n  /// after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```\n  /// x\n  /// `````\n  ///\n  /// after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ~~~\n  /// ```\n  /// ~~~\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ~~~\n  /// ```\n  /// ~~~\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```\n  /// x\n  /// ``\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```\n  /// x\n  /// ``\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ``` a`b\n  /// not a fence with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ``` a`b not a fence with enough words after it that the formatter has to wrap this line onto\n  /// the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ~~~~ java  \n  /// int x;\n  /// ~~~~\n  /// after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ~~~~java\n  /// int x;\n  /// ~~~~\n  ///\n  /// after with enough words after it that the formatter has to wrap this line onto the next one\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// Before with enough words after it that the formatter has to wrap this line onto the next one\n  /// ```\n  /// code   kept\n  ///   indented\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Before with enough words after it that the formatter has to wrap this line onto the next one\n  ///\n  /// ```\n  /// code   kept\n  ///   indented\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// ```\n  /// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// ```\n  /// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n  /// ```\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > quoted\n  ///\n  /// paragraph after with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > quoted\n  ///\n  /// paragraph after with enough words after it that the formatter has to wrap this quote onto the\n  /// next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > first paragraph with enough words after it that the formatter has to wrap this quote onto the next line\n  /// >\n  /// > second paragraph\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > first paragraph with enough words after it that the formatter has to wrap this quote onto\n  /// > the next line\n  /// >\n  /// > second paragraph\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > a `code\n  /// > span` across with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > a `code span` across with enough words after it that the formatter has to wrap this quote\n  /// > onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > > > deep with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >\n  /// > > > deep with enough words after it that the formatter has to wrap this quote onto the next\n  /// > > > line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > quoted\n  ///\n  /// @param x the value with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > quoted\n  ///\n  /// @param x the value with enough words after it that the formatter has to wrap this quote onto\n  ///   the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > # heading\n  /// > text with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >\n  /// >\n  /// > # heading\n  /// >\n  /// > text with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  ///   > indented quote with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > indented quote with enough words after it that the formatter has to wrap this quote onto the\n  /// > next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// >     code in quote\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >     code in quote\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > quoted text with enough words after it that the formatter has to wrap this quote onto the next line\n  /// lazy continuation\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > quoted text with enough words after it that the formatter has to wrap this quote onto the\n  /// > next line lazy continuation\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > [a](b\n  /// > ) `c` d\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > [a](b ) `c` d\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item\n  ///\n  ///   > quoted with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item\n  ///\n  ///   > quoted with enough words after it that the formatter has to wrap this quote onto the next\n  ///   > line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > outer with enough words after it that the formatter has to wrap this quote onto the next line\n  /// > > inner with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > outer with enough words after it that the formatter has to wrap this quote onto the next\n  /// > line\n  /// > > inner with enough words after it that the formatter has to wrap this quote onto the next\n  /// > > line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// >tight quote with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > tight quote with enough words after it that the formatter has to wrap this quote onto the\n  /// > next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > a\n  /// >\n  /// >\n  /// > b\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// > a\n  /// >\n  /// > b\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// > - one with enough words after it that the formatter has to wrap this quote onto the next line\n  /// > - two\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >\n  /// > - one with enough words after it that the formatter has to wrap this quote onto the next\n  /// >   line\n  /// > - two\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// >\tquote after tab with enough words after it that the formatter has to wrap this quote onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// >  quote after tab with enough words after it that the formatter has to wrap this quote onto\n  /// > the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - a   item\n  ///  \tcontinued   text\n  void f() {}\n}\n",
                "class A {\n  /// - a item continued text\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// - a   item\n  ///     continued   text\n  void f() {}\n}\n",
                "class A {\n  /// - a item continued text\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// - a   item\n  ///\tcontinued   text\n  void f() {}\n}\n",
                "class A {\n  /// - a item continued text\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Intro.\n  ///\n  /// - one with enough words after it that the formatter has to wrap this item onto the next line\n  ///\n  /// - two\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Intro.\n  ///\n  /// - one with enough words after it that the formatter has to wrap this item onto the next line\n  ///\n  /// - two\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - outer with enough words after it that the formatter has to wrap this item onto the next line\n  ///   - inner with enough words after it that the formatter has to wrap this item onto the next line\n  ///   - second inner\n  /// - outer two\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - outer with enough words after it that the formatter has to wrap this item onto the next line\n  ///   - inner with enough words after it that the formatter has to wrap this item onto the next\n  ///     line\n  ///   - second inner\n  /// - outer two\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// Text before\n  /// 3. not a list\n  /// 4. still text\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Text before 3. not a list 4. still text\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// Text before\n  /// 1. a list with enough words after it that the formatter has to wrap this item onto the next line\n  /// 2. second\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// Text before\n  /// 1. a list with enough words after it that the formatter has to wrap this item onto the next\n  ///    line\n  /// 2. second\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - first paragraph with enough words after it that the formatter has to wrap this item onto the next line\n  ///\n  ///   second paragraph in the item with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - first paragraph with enough words after it that the formatter has to wrap this item onto the\n  ///   next line\n  ///\n  ///   second paragraph in the item with enough words after it that the formatter has to wrap this\n  ///   item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item text with enough words after it that the formatter has to wrap this item onto the next line\n  /// lazy continuation line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item text with enough words after it that the formatter has to wrap this item onto the next\n  ///   line lazy continuation line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - a\n  /// + b\n  /// * c\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - a\n  /// + b\n  /// * c\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// 1. a\n  /// 1) b\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// 1. a\n  /// 1) b\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// -     indented code in an item\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// -     indented code in an item\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item with `code span` inside with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item with `code span` inside with enough words after it that the formatter has to wrap this\n  ///   item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - item\n  ///\n  /// paragraph after the list with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - item\n  ///\n  /// paragraph after the list with enough words after it that the formatter has to wrap this item\n  /// onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - a\n  ///\n  ///  b\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - a\n  ///\n  /// b\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - a list item\n  ///\n  /// @param x the parameter with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - a list item\n  ///\n  /// @param x the parameter with enough words after it that the formatter has to wrap this item\n  ///   onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// 10. ten with enough words after it that the formatter has to wrap this item onto the next line\n  /// 11. eleven\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// 10. ten with enough words after it that the formatter has to wrap this item onto the next line\n  /// 11. eleven\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// - ***\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// - ***\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// * * *\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// * * *\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// -   spaced marker with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// -  spaced marker with enough words after it that the formatter has to wrap this item onto the\n  ///   next line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// * star item with enough words after it that the formatter has to wrap this item onto the next line\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// * star item with enough words after it that the formatter has to wrap this item onto the next\n  ///   line\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b`c) d`e\n  /// @param x` text\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b`c) d`e @param x` text\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [![a](b)](c`d) e`f\n  /// @param x` g\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [![a](b)](c`d) e`f @param x` g\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See ![[a](b)](c`d) e`f\n  /// @param x` g\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See ![[a](b)](c`d) e`f @param x` g\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b(((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))`c) d`e\n  /// @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b(((((((((((((((((((((((((((((((())))))))))))))))))))))))))))))))`c) d`e @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b((((((((((((((((((((((((((((((((()))))))))))))))))))))))))))))))))`c) d`e\n  /// @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b((((((((((((((((((((((((((((((((()))))))))))))))))))))))))))))))))`c) d`e\n  ///\n  /// @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [[a](b) c](d`e) f`g\n  /// @param x` h\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [[a](b) c](d`e) f`g\n  ///\n  /// @param x` h\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b (t`u(v) w`x)\n  /// @param x` z\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b (t`u(v) w`x)\n  ///\n  /// @param x` z\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b \"t`u\") v`w\n  /// @param x` y\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b \"t`u\") v`w @param x` y\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b`c d`e\n  /// @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b`c d`e\n  ///\n  /// @param x` f\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  /// See [a](b \"t`c\n  /// @param x`\") text\n  int f(int x) {\n    return x;\n  }\n}\n",
                "class A {\n  /// See [a](b \"t`c\n  ///\n  /// @param x`\") text\n  int f(int x) {\n    return x;\n  }\n}\n",
            ),
            (
                "class A {\n  ///   Ends   with `code`\n  void f() {}\n}\n",
                "class A {\n  /// Ends with `code`\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Use `a` and ``b`c`` here, and `unclosed text that runs on filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// Use `a` and ``b`c`` here, and `unclosed text that runs on filler words to push the text well\n  /// past the column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Escaped \\`not code\\` and `real code span here` with more filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// Escaped \\`not code\\` and `real code span here` with more filler words to push the text well\n  /// past the column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Doubled `` `x` `` ticks filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// Doubled `` `x` `` ticks filler words to push the text well past the column limit of one\n  /// hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Starts a span `across\n  /// lines with` more words filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// Starts a span `across lines with` more words filler words to push the text well past the\n  /// column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// A tag `{@code x}` in a span and {@code y} outside filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// A tag `{@code x}` in a span and {@code y} outside filler words to push the text well past the\n  /// column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Text `a\n  /// @param b` still code filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// Text `a @param b` still code filler words to push the text well past the column limit of one\n  /// hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// [`List`](https://example.com/list) is a link filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// [`List`](https://example.com/list) is a link filler words to push the text well past the\n  /// column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// foo`bar`baz glued together and then the rest of the sentence goes on filler words to push the text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
                "class A {\n  /// foo`bar`baz glued together and then the rest of the sentence goes on filler words to push the\n  /// text well past the column limit of one hundred characters here\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// First line\\\n  /// second line with \\* an escaped star and a long tail that makes this line wrap past the column limit.\n  void f() {}\n}\n",
                "class A {\n  /// First line\\\n  /// second line with \\* an escaped star and a long tail that makes this line wrap past the column\n  /// limit.\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Does it.\n  ///\n  /// @param a the first value, described at a length that goes past the column limit of the line here\n  ///\n  ///   still the first value\n  /// @return nothing\n  int f(int a) {\n    return 0;\n  }\n}\n",
                "class A {\n  /// Does it.\n  ///\n  /// @param a the first value, described at a length that goes past the column limit of the line\n  ///   here\n  ///\n  ///   still the first value\n  /// @return nothing\n  int f(int a) {\n    return 0;\n  }\n}\n",
            ),
            (
                "class A {\n  ///  Text {@code open\n  ///    more\n  void f() {}\n}\n",
                "class A {\n  ///  Text {@code open\n  ///    more\n  void f() {}\n}\n",
            ),
            (
                "class A {\n  /// Ends with a backslash \\\n  void f() {}\n\n  /// Short.\n  void g() {}\n}\n",
                "class A {\n  /// Ends with a backslash \\\n  void f() {}\n\n  /// Short.\n  void g() {}\n}\n",
            ),
        ];
        for (input, expected) in cases {
            let unit = parser::parse(input).ok();

            let formatted = unit.map(|unit| format(input, &unit));

            assert_eq!(formatted, Some(Ok(expected.to_owned())), "{input}");
        }
    }
}
