use super::lexer::{Kind, SPACE, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Requested {
    None,
    Whitespace,
    Newline,
    BlankLine,
}

struct Writer {
    block_indent: usize,
    max_line_length: usize,
    output: String,
    continuing_footer_tag: bool,
    remaining_on_line: isize,
    at_start_of_line: bool,
    requested: Requested,
    wrote_anything_significant: bool,
}

impl Writer {
    fn newline(&mut self) {
        self.output.push('\n');
        self.output.push_str(&" ".repeat(self.block_indent + 1));
        self.output.push_str("* ");
        let indent = match self.continuing_footer_tag {
            true => 4,
            false => 0,
        };
        self.output.push_str(&" ".repeat(indent));
        let width = self.max_line_length.cast_signed() - self.block_indent.cast_signed() - 3;
        self.remaining_on_line = width - indent.cast_signed();
        self.at_start_of_line = true;
    }

    fn flush_whitespace(&mut self) {
        let significant = match self.wrote_anything_significant {
            true => self.requested,
            false => Requested::None,
        };
        let requested = match (significant, self.continuing_footer_tag) {
            (Requested::BlankLine, true) => Requested::Newline,
            (requested, _) => requested,
        };
        self.requested = match requested {
            Requested::BlankLine => {
                self.output.push('\n');
                self.output.push_str(&" ".repeat(self.block_indent + 1));
                self.output.push('*');
                self.newline();
                Requested::None
            }
            Requested::Newline => {
                self.newline();
                Requested::None
            }
            Requested::None | Requested::Whitespace => requested,
        };
    }

    fn write_token(&mut self, token: &Token) {
        self.flush_whitespace();
        let need_whitespace = self.requested == Requested::Whitespace;
        let length = token.value.encode_utf16().count().cast_signed();
        let space = isize::from(need_whitespace);
        if !self.at_start_of_line && length + space > self.remaining_on_line {
            self.newline();
        }
        if !self.at_start_of_line && need_whitespace {
            self.output.push(' ');
            self.remaining_on_line -= 1;
        }
        self.output.push_str(&token.value);
        self.at_start_of_line &= token.kind == Kind::ParagraphOpen;
        self.remaining_on_line -= length;
        self.requested = Requested::None;
        self.wrote_anything_significant = true;
    }
}

pub fn render(tokens: &[Token], block_indent: usize, max_line_length: usize) -> String {
    let mut writer = Writer {
        block_indent,
        max_line_length,
        output: String::from("/**"),
        continuing_footer_tag: false,
        remaining_on_line: 0,
        at_start_of_line: false,
        requested: Requested::None,
        wrote_anything_significant: false,
    };
    writer.newline();
    for token in tokens {
        match token.kind {
            Kind::BeginJavadoc | Kind::ParagraphClose => {}
            Kind::EndJavadoc => {
                writer.output.push('\n');
                writer.output.push_str(&" ".repeat(block_indent + 1));
                writer.output.push_str("*/");
            }
            Kind::FooterTagStart => {
                writer.requested = writer.requested.max(match writer.continuing_footer_tag {
                    true => Requested::Newline,
                    false => Requested::BlankLine,
                });
                writer.continuing_footer_tag = false;
                writer.write_token(token);
                writer.continuing_footer_tag = true;
            }
            Kind::ParagraphOpen if writer.wrote_anything_significant => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                let simple = token.value.strip_prefix('<').and_then(|tail| {
                    let name =
                        tail.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
                    let rest = name.trim_start_matches(SPACE);
                    let rest = rest.strip_prefix('/').unwrap_or(rest);
                    (name.len() < tail.len()).then_some(rest.trim_start_matches(SPACE) == ">")
                });
                let standard = Token::from((Kind::ParagraphOpen, "<p>"));
                match simple {
                    Some(true) => writer.write_token(&standard),
                    Some(false) | None => writer.write_token(token),
                }
            }
            Kind::ParagraphOpen => {}
            Kind::Whitespace => writer.requested = writer.requested.max(Requested::Whitespace),
            Kind::Literal => writer.write_token(token),
        }
    }
    let output = writer.output;
    let single = output
        .strip_prefix("/**\n")
        .and_then(|rest| rest.split_once('\n'))
        .and_then(|(line, last)| {
            let content = line.trim_start_matches(' ').strip_prefix("* ")?;
            let terminated = content.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']);
            (last.trim_start_matches(' ') == "*/" && !terminated).then_some(content)
        });
    let fits = |line: &str| {
        let content = max_line_length.saturating_sub("/**  */".len() + block_indent);
        line.encode_utf16().count() <= content && (!line.starts_with('@') || line == "@hide")
    };
    match single {
        Some("") => String::from("/** */"),
        Some(line) if fits(line) => format!("/** {line} */"),
        Some(_) | None => output,
    }
}
