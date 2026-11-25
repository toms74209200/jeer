use super::lexer::{Kind, SPACE, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Requested {
    None,
    Whitespace,
    Newline,
    BlankLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Indent {
    List(usize),
    ListItem(usize),
}

struct Writer {
    block_indent: usize,
    max_line_length: usize,
    output: String,
    continuing_list_item: bool,
    continuing_footer_tag: bool,
    indents: Vec<Indent>,
    modified_lists: usize,
    remaining_on_line: isize,
    at_start_of_line: bool,
    requested: Requested,
    wrote_anything_significant: bool,
    moe_begin: Option<String>,
    moe_end_indent: String,
}

impl Writer {
    fn line_start(&mut self) {
        self.output.push('\n');
        self.output.push_str(&" ".repeat(self.block_indent + 1));
        self.output.push_str("* ");
        let width = self.max_line_length.cast_signed() - self.block_indent.cast_signed() - 3;
        self.remaining_on_line = width;
        self.at_start_of_line = true;
    }

    fn inner_indent(&self) -> String {
        let footer = match self.continuing_footer_tag {
            true => 4,
            false => 0,
        };
        let indents: usize = self
            .indents
            .iter()
            .map(|indent| match indent {
                Indent::List(width) | Indent::ListItem(width) => *width,
            })
            .sum();
        " ".repeat(indents + footer)
    }

    fn newline(&mut self) {
        self.line_start();
        let indent = self.inner_indent();
        self.output.push_str(&indent);
        self.remaining_on_line -= indent.len().cast_signed();
    }

    fn flush_whitespace(&mut self) {
        if self.moe_begin.is_some() {
            self.requested = self.requested.max(Requested::Newline);
        }
        let significant = match self.wrote_anything_significant {
            true => self.requested,
            false => Requested::None,
        };
        let continuing = self.modified_lists > 0 || self.continuing_footer_tag;
        let requested = match (significant, continuing) {
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
        if let Some(moe) = self.moe_begin.take() {
            self.output.push_str(&moe);
            self.moe_end_indent = self.inner_indent();
            self.wrote_anything_significant = true;
            self.requested = self.requested.max(Requested::Newline);
            self.write_token(token);
            return;
        }
        self.output.push_str(&token.value);
        self.at_start_of_line &= matches!(
            token.kind,
            Kind::ParagraphOpen | Kind::ListItemOpen | Kind::HeaderOpen | Kind::BlockQuoteOpen
        );
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
        continuing_list_item: false,
        continuing_footer_tag: false,
        indents: Vec::new(),
        modified_lists: 0,
        remaining_on_line: 0,
        at_start_of_line: false,
        requested: Requested::None,
        wrote_anything_significant: false,
        moe_begin: None,
        moe_end_indent: String::new(),
    };
    let simple = |value: &str| {
        value
            .strip_prefix('<')
            .and_then(|tail| {
                let name = tail.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
                let rest = name.trim_start_matches(SPACE);
                let rest = rest.strip_prefix('/').unwrap_or(rest);
                (name.len() < tail.len()).then_some(rest.trim_start_matches(SPACE) == ">")
            })
            .unwrap_or(false)
    };
    writer.newline();
    for token in tokens {
        match token.kind {
            Kind::BeginJavadoc
            | Kind::ParagraphClose
            | Kind::ListItemClose
            | Kind::OptionalLineBreak => {}
            Kind::SnippetBegin | Kind::HeaderOpen | Kind::TableOpen => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                writer.write_token(token);
            }
            Kind::SnippetEnd | Kind::HeaderClose | Kind::TableClose => {
                writer.write_token(token);
                writer.requested = writer.requested.max(Requested::BlankLine);
            }
            Kind::BlockQuoteOpen => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                writer.write_token(token);
                writer.requested = writer.requested.max(Requested::Newline);
            }
            Kind::BlockQuoteClose => {
                writer.requested = writer.requested.max(Requested::Newline);
                writer.write_token(token);
                writer.requested = writer.requested.max(Requested::BlankLine);
            }
            Kind::Br => {
                let standard = Token::from((Kind::Br, "<br>"));
                match simple(&token.value) {
                    true => writer.write_token(&standard),
                    false => writer.write_token(token),
                }
                writer.requested = writer.requested.max(Requested::Newline);
            }
            Kind::MoeBegin => writer.moe_begin = Some(token.value.clone()),
            Kind::MoeEnd => {
                writer.requested = Requested::None;
                writer.line_start();
                let indent = writer.moe_end_indent.clone();
                writer.output.push_str(&indent);
                writer.write_token(token);
                writer.requested = writer.requested.max(Requested::Newline);
            }
            Kind::HtmlComment => {
                writer.requested = writer.requested.max(Requested::Newline);
                let mut lines = token.value.lines();
                let first = lines.next().unwrap_or_default();
                writer.write_token(&Token::from((Kind::HtmlComment, first)));
                for line in lines {
                    writer.line_start();
                    writer.output.push_str(line);
                    writer.remaining_on_line -= line.encode_utf16().count().cast_signed();
                }
                writer.requested = writer.requested.max(Requested::Newline);
            }
            Kind::EndJavadoc => {
                writer.output.push('\n');
                writer.output.push_str(&" ".repeat(block_indent + 1));
                writer.output.push_str("*/");
            }
            Kind::FooterTagStart => {
                writer.continuing_list_item = false;
                writer.indents.clear();
                writer.modified_lists = 0;
                writer.requested = writer.requested.max(match writer.continuing_footer_tag {
                    true => Requested::Newline,
                    false => Requested::BlankLine,
                });
                writer.continuing_footer_tag = false;
                writer.write_token(token);
                writer.continuing_footer_tag = true;
            }
            Kind::ListOpen => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                writer.write_token(token);
                writer.continuing_list_item = false;
                writer.indents.push(Indent::List(2));
                writer.modified_lists += 1;
                writer.requested = writer.requested.max(Requested::Newline);
            }
            Kind::ListClose => {
                writer.requested = writer.requested.max(Requested::Newline);
                let list = writer
                    .indents
                    .iter()
                    .rposition(|indent| matches!(indent, Indent::List(_)));
                writer
                    .indents
                    .truncate(list.unwrap_or(writer.indents.len()));
                writer.write_token(token);
                writer.modified_lists = writer.modified_lists.saturating_sub(1);
                writer.requested = writer.requested.max(Requested::BlankLine);
            }
            Kind::ListItemOpen => {
                writer.requested = writer.requested.max(Requested::Newline);
                if writer.continuing_list_item {
                    writer.continuing_list_item = false;
                    let item = writer
                        .indents
                        .iter()
                        .rposition(|indent| matches!(indent, Indent::ListItem(_)));
                    writer
                        .indents
                        .truncate(item.unwrap_or(writer.indents.len()));
                }
                writer.write_token(token);
                writer.continuing_list_item = true;
                let width = token.value.encode_utf16().count();
                writer.indents.push(Indent::ListItem(width));
            }
            Kind::ParagraphOpen if writer.wrote_anything_significant => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                let standard = Token::from((Kind::ParagraphOpen, "<p>"));
                match simple(&token.value) {
                    true => writer.write_token(&standard),
                    false => writer.write_token(token),
                }
            }
            Kind::ParagraphOpen => {}
            Kind::PreOpen => {
                writer.requested = writer.requested.max(Requested::BlankLine);
                writer.write_token(token);
            }
            Kind::PreClose => {
                writer.write_token(token);
                writer.requested = writer.requested.max(Requested::BlankLine);
            }
            Kind::CodeOpen | Kind::CodeClose | Kind::Literal => writer.write_token(token),
            Kind::ForcedNewline => writer.line_start(),
            Kind::Whitespace => writer.requested = writer.requested.max(Requested::Whitespace),
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
