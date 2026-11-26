use crate::lexer::token::{Punct, TokenKind};
use crate::parser::syntax::{Element, Node, NodeKind};
use std::collections::VecDeque;

const COLUMN_LIMIT: usize = 100;

struct Literal<'a> {
    node: &'a Node,
    enclosing: &'a Node,
}

fn string_literal(node: &Node) -> Option<(usize, usize)> {
    match (node.kind, node.children.as_slice()) {
        (NodeKind::Literal, [Element::Token(token)]) if token.kind == TokenKind::StringLiteral => {
            Some((token.start, token.end))
        }
        _ => None,
    }
}

fn literal_span(node: &Node) -> Option<(usize, usize)> {
    match (node.kind, node.children.as_slice()) {
        (NodeKind::Literal, [Element::Token(token)])
            if matches!(token.kind, TokenKind::StringLiteral | TokenKind::TextBlock) =>
        {
            Some((token.start, token.end))
        }
        _ => None,
    }
}

fn plus(node: &Node) -> Option<(&Node, &Node)> {
    match (node.kind, node.children.as_slice()) {
        (
            NodeKind::Binary,
            [
                Element::Node(left),
                Element::Token(operator),
                Element::Node(right),
            ],
        ) if operator.kind == TokenKind::Punct(Punct::Plus) => Some((left, right)),
        _ => None,
    }
}

fn column(input: &str, position: usize) -> usize {
    let line_start = input[..position].rfind('\n').map_or(0, |index| index + 1);
    input[line_start..position].encode_utf16().count()
}

fn line_end(input: &str, position: usize) -> usize {
    let rest = &input[position..];
    rest.find(['\n', '\r'])
        .map_or(input.len(), |offset| position + offset)
}

fn long_literals<'a>(input: &str, root: &'a Node) -> Vec<Literal<'a>> {
    let mut literals: Vec<Literal> = Vec::new();
    let mut pending: Vec<(&Node, Option<&Node>)> = vec![(root, None)];
    while let Some((node, chain)) = pending.pop() {
        let below = plus(node).map(|_| chain.unwrap_or(node));
        let receiver = match (node.kind, node.children.first()) {
            (NodeKind::MethodCall | NodeKind::FieldAccess, Some(Element::Node(first))) => {
                Some(std::ptr::from_ref(first))
            }
            _ => None,
        };
        for child in node.children.iter().rev().filter_map(|child| match child {
            Element::Node(child) => Some(child),
            Element::Token(_) => None,
        }) {
            let long = string_literal(child)
                .filter(|_| receiver != Some(std::ptr::from_ref(child)))
                .is_some_and(|(_, end)| column(input, line_end(input, end)) > COLUMN_LIMIT);
            if long {
                literals.push(Literal {
                    node: child,
                    enclosing: below.unwrap_or(child),
                });
            }
            pending.push((child, below));
        }
    }
    literals.reverse();
    literals
}

fn operands(enclosing: &Node) -> Vec<&Node> {
    let mut flat: Vec<&Node> = Vec::new();
    let mut todo = vec![enclosing];
    while let Some(next) = todo.pop() {
        match plus(next) {
            Some((left, right)) => {
                todo.push(right);
                todo.push(left);
            }
            None => flat.push(next),
        }
    }
    flat
}

fn replacement(input: &str, literal: &Literal) -> Option<((usize, usize), String)> {
    let flat = operands(literal.enclosing);
    let index = flat
        .iter()
        .position(|node| std::ptr::eq(*node, literal.node))?;
    let joinable = |left: &Node, right: &Node| match (literal_span(left), literal_span(right)) {
        (Some((_, end)), Some((start, _))) => input[end..start]
            .chars()
            .all(|c| c.is_whitespace() || c == '"' || c == '+'),
        _ => false,
    };
    let start_index = (0..index)
        .rev()
        .take_while(|&before| joinable(flat[before], flat[before + 1]))
        .last()
        .unwrap_or(index);
    let end_index = (index + 1..flat.len())
        .take_while(|&after| joinable(flat[after - 1], flat[after]))
        .last()
        .map_or(index + 1, |after| after + 1);
    let chosen: Vec<(usize, usize)> = flat[start_index..end_index]
        .iter()
        .filter_map(|operand| literal_span(operand))
        .collect();
    let (first_start, _) = chosen[0];
    let (_, last_end) = chosen[chosen.len() - 1];
    let trailing = input[last_end..line_end(input, last_end)]
        .encode_utf16()
        .count();
    let texts = chosen
        .iter()
        .map(|&(start, end)| &input[start + 1..end - 1]);
    let text = reflow(
        column(input, first_start),
        trailing,
        &string_components(texts),
        start_index == 0,
    );
    Some(((first_start, last_end), text))
}

pub fn wrap(input: &str, root: &Node) -> String {
    let computed = long_literals(input, root)
        .iter()
        .filter_map(|literal| replacement(input, literal))
        .collect::<Vec<_>>();
    let mut replacements: Vec<((usize, usize), String)> = Vec::new();
    for (range, text) in computed {
        replacements.retain(|(existing, _)| *existing != range);
        replacements.push((range, text));
    }
    replacements.sort_by_key(|((start, _), _)| std::cmp::Reverse(*start));
    let mut output = input.to_owned();
    for ((start, end), text) in replacements {
        output.replace_range(start..end, &text);
    }
    output
}

fn split_points(chars: &[char]) -> Vec<usize> {
    let escaped =
        |at: usize, what: char| chars.get(at) == Some(&'\\') && chars.get(at + 1) == Some(&what);
    let newline = |at: usize| match (escaped(at, 'r'), escaped(at + 2, 'n'), escaped(at, 'n')) {
        (true, true, _) => 4,
        (true, false, _) | (false, _, true) => 2,
        (false, _, false) => 0,
    };
    let mut points = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match (
            chars[index].is_whitespace(),
            escaped(index, 't'),
            newline(index),
        ) {
            (true, _, _) => {
                points.push(index);
                index += 1;
            }
            (false, true, _) => {
                points.push(index);
                index += 2;
            }
            (false, false, 0) => {
                index += match chars[index] {
                    '\\' => 2,
                    _ => 1,
                };
            }
            (false, false, _) => {
                while newline(index) > 0 {
                    index += newline(index);
                }
                points.push(index);
            }
        }
    }
    points
}

fn string_components<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut piece = String::new();
    for text in texts {
        let chars: Vec<char> = text.chars().collect();
        let start = split_points(&chars).into_iter().fold(0, |start, split| {
            match split > start || !piece.is_empty() {
                true => {
                    piece.extend(&chars[start..split]);
                    result.push(std::mem::take(&mut piece));
                    split
                }
                false => start,
            }
        });
        if !piece.is_empty() {
            result.push(std::mem::take(&mut piece));
        }
        piece.extend(&chars[start..]);
    }
    if !piece.is_empty() {
        result.push(piece);
    }
    result
}

struct Reflowed {
    width: isize,
    continuing: bool,
    lines: Vec<String>,
}

fn take_line(input: &mut VecDeque<&str>, width: isize) -> String {
    let mut line = String::new();
    while let Some(&text) = input.front() {
        let used = line.encode_utf16().count().cast_signed();
        if used > 4 && used + text.encode_utf16().count().cast_signed() > width {
            break;
        }
        input.pop_front();
        line.push_str(text);
        if text.ends_with("\\n") || text.ends_with("\\r") {
            break;
        }
    }
    line
}

fn reflow(start_column: usize, trailing: usize, components: &[String], first: bool) -> String {
    let mut input: VecDeque<&str> = components.iter().map(String::as_str).collect();
    let mut state = Reflowed {
        width: COLUMN_LIMIT.cast_signed() - start_column.cast_signed() - 2,
        continuing: first,
        lines: Vec::new(),
    };
    while !input.is_empty() {
        let total: usize = input.iter().map(|text| text.encode_utf16().count()).sum();
        if total.cast_signed() <= state.width {
            state.width -= trailing.cast_signed();
        }
        let line = take_line(&mut input, state.width);
        state.lines.push(line);
        if state.continuing {
            state.width -= 6;
            state.continuing = false;
        }
    }
    let indent = match first {
        true => start_column + 4,
        false => start_column.saturating_sub(2),
    };
    let separator = format!("\"\n{}+ \"", " ".repeat(indent));
    format!("\"{}\"", state.lines.join(&separator))
}
