use std::collections::HashMap;

use super::doc::{BreakTag, Range, Sink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlankLine {
    Yes,
    No,
    Preserve,
    Conditional(Vec<BreakTag>),
}

impl BlankLine {
    pub fn merge(self, other: BlankLine) -> BlankLine {
        match (self, other) {
            (BlankLine::Conditional(mut tags), BlankLine::Conditional(more)) => {
                tags.extend(more);
                BlankLine::Conditional(tags)
            }
            (BlankLine::Conditional(_), other) => other,
            (simple @ (BlankLine::Yes | BlankLine::No | BlankLine::Preserve), _) => simple,
        }
    }

    fn wanted(&self, taken: &[bool]) -> Option<bool> {
        match self {
            BlankLine::Yes => Some(true),
            BlankLine::No => Some(false),
            BlankLine::Preserve => None,
            BlankLine::Conditional(tags) => tags
                .iter()
                .any(|tag| taken.get(tag.0) == Some(&true))
                .then_some(true),
        }
    }
}

#[derive(Debug, Default)]
pub struct Pending {
    input_line: usize,
    last: usize,
    lines: Vec<String>,
    line: String,
    spaces: String,
    newlines: usize,
}

#[derive(Debug, Default)]
pub struct JavaOutput {
    pub input_lines: Vec<Range>,
    pub blank_lines: HashMap<usize, BlankLine>,
    pub taken: Vec<bool>,
    pub pending: Pending,
}

impl Sink for JavaOutput {
    fn append(&mut self, text: &str, range: Range) {
        if let Some((low, _)) = range {
            let mut saw_newlines = false;
            while let Some(line) = self.input_lines.get(self.pending.input_line) {
                match line {
                    None => saw_newlines = true,
                    Some((_, high)) if *high <= low => {}
                    Some(_) => break,
                }
                self.pending.input_line += 1;
            }
            let wanted = self
                .blank_lines
                .get(&self.pending.last)
                .map_or(Some(false), |blank| blank.wanted(&self.taken));
            let comment = text.starts_with("//") || text.starts_with("/*");
            if (saw_newlines && comment) || wanted.unwrap_or(saw_newlines) {
                self.pending.newlines += 1;
            }
        }
        if let "\n" | "\r\n" | "\r" = text {
            self.pending.newlines = self.pending.newlines.max(1);
            self.pending.spaces.clear();
        } else {
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    ' ' | '\t' => self.pending.spaces.push(c),
                    '\r' | '\n' => {
                        if c == '\r' && chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        self.pending.spaces.clear();
                        self.pending.newlines += 1;
                    }
                    _ => {
                        while self.pending.newlines > 0 {
                            if !self.pending.lines.is_empty() || !self.pending.line.is_empty() {
                                self.pending
                                    .lines
                                    .push(std::mem::take(&mut self.pending.line));
                            }
                            self.pending.newlines -= 1;
                        }
                        self.pending.line.push_str(&self.pending.spaces);
                        self.pending.spaces.clear();
                        self.pending.line.push(c);
                    }
                }
            }
        }
        if let Some((_, high)) = range {
            self.pending.last = high;
        }
    }

    fn indent(&mut self, indent: usize) {
        self.pending.spaces.push_str(&" ".repeat(indent));
    }
}

impl From<JavaOutput> for String {
    fn from(output: JavaOutput) -> Self {
        let mut lines = output.pending.lines;
        if !output.pending.line.chars().all(char::is_whitespace) {
            lines.push(output.pending.line);
        }
        lines.iter().map(|line| format!("{line}\n")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_append_with_text_newlines_and_indents_then_returns_lines_as_java_output_does() {
        let cases: [(&[(&str, usize)], &str); 5] = [
            (&[("a", 0), ("\n", 4), ("b", 0)], "a\n    b\n"),
            (&[("a", 0), ("\n", 0), ("\n", 0), ("b", 0)], "a\nb\n"),
            (&[("a  ", 0), ("\n", 0), ("b", 0)], "a\nb\n"),
            (&[("\n", 0), ("a\n\nb", 0), ("  ", 0)], "a\n\nb\n"),
            (&[("a\r\nb\rc\t", 0)], "a\nb\nc\n"),
        ];
        for (appends, expected) in cases {
            let mut sink = JavaOutput::default();
            for (text, indent) in appends {
                sink.append(text, None);
                sink.indent(*indent);
            }

            let output = String::from(sink);

            assert_eq!(output, expected, "{appends:?}");
        }
    }

    #[test]
    fn when_append_with_blank_line_requests_then_returns_blank_lines_as_java_output_does() {
        let cases = [
            (vec![Some((0, 1)), None, Some((1, 2))], None, "a\nb\n"),
            (
                vec![Some((0, 1)), None, Some((1, 2))],
                Some(BlankLine::Preserve),
                "a\n\nb\n",
            ),
            (
                vec![Some((0, 1)), None, Some((1, 2))],
                Some(BlankLine::No),
                "a\nb\n",
            ),
            (
                vec![Some((0, 1)), Some((1, 2))],
                Some(BlankLine::Yes),
                "a\n\nb\n",
            ),
            (
                vec![Some((0, 1)), Some((1, 2))],
                Some(BlankLine::Preserve),
                "a\nb\n",
            ),
            (
                vec![Some((0, 1)), Some((1, 2))],
                Some(BlankLine::Conditional(vec![BreakTag(0)])),
                "a\n\nb\n",
            ),
            (
                vec![Some((0, 1)), Some((1, 2))],
                Some(BlankLine::Conditional(vec![BreakTag(1)])),
                "a\nb\n",
            ),
        ];
        for (input_lines, blank, expected) in cases {
            let blank_lines = blank.iter().map(|b| (1, b.clone())).collect();
            let mut output = JavaOutput {
                input_lines: input_lines.clone(),
                blank_lines,
                taken: vec![true, false],
                ..JavaOutput::default()
            };

            output.append("a", Some((0, 1)));
            output.append("\n", None);
            output.append("b", Some((1, 2)));

            assert_eq!(String::from(output), expected, "{input_lines:?} {blank:?}");
        }
    }

    #[test]
    fn when_merge_with_blank_lines_then_returns_the_request_gjf_keeps() {
        let cases = [
            (BlankLine::Yes, BlankLine::No, BlankLine::Yes),
            (BlankLine::Preserve, BlankLine::Yes, BlankLine::Preserve),
            (
                BlankLine::Conditional(vec![BreakTag(0)]),
                BlankLine::No,
                BlankLine::No,
            ),
            (
                BlankLine::Conditional(vec![BreakTag(0)]),
                BlankLine::Conditional(vec![BreakTag(1)]),
                BlankLine::Conditional(vec![BreakTag(0), BreakTag(1)]),
            ),
        ];
        for (first, second, expected) in cases {
            let merged = first.clone().merge(second.clone());

            assert_eq!(merged, expected, "{first:?} {second:?}");
        }
    }
}
