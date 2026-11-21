use super::doc::Sink;

#[derive(Debug, Default)]
pub struct TextSink {
    lines: Vec<String>,
    line: String,
    spaces: String,
    newlines: usize,
}

impl Sink for TextSink {
    fn append(&mut self, text: &str) {
        if let "\n" | "\r\n" | "\r" = text {
            self.newlines = self.newlines.max(1);
            self.spaces.clear();
            return;
        }
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                ' ' | '\t' => self.spaces.push(c),
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    self.spaces.clear();
                    self.newlines += 1;
                }
                _ => {
                    while self.newlines > 0 {
                        if !self.lines.is_empty() || !self.line.is_empty() {
                            self.lines.push(std::mem::take(&mut self.line));
                        }
                        self.line.clear();
                        self.newlines -= 1;
                    }
                    self.line.push_str(&self.spaces);
                    self.spaces.clear();
                    self.line.push(c);
                }
            }
        }
    }

    fn indent(&mut self, indent: usize) {
        self.spaces.push_str(&" ".repeat(indent));
    }
}

impl From<TextSink> for String {
    fn from(sink: TextSink) -> Self {
        let mut lines = sink.lines;
        if !sink.line.chars().all(char::is_whitespace) {
            lines.push(sink.line);
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
            let mut sink = TextSink::default();
            for (text, indent) in appends {
                sink.append(text);
                sink.indent(*indent);
            }

            let output = String::from(sink);

            assert_eq!(output, expected, "{appends:?}");
        }
    }
}
