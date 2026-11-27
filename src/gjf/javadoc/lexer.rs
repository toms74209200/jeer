#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BeginJavadoc,
    EndJavadoc,
    FooterTagStart,
    ListOpen,
    ListClose,
    ListItemOpen,
    ListItemClose,
    SnippetBegin,
    SnippetEnd,
    TableOpen,
    TableClose,
    BlockQuoteOpen,
    BlockQuoteClose,
    HeaderOpen,
    HeaderClose,
    Br,
    MoeBegin,
    MoeEnd,
    HtmlComment,
    OptionalLineBreak,
    ParagraphOpen,
    ParagraphClose,
    PreOpen,
    PreClose,
    CodeOpen,
    CodeClose,
    Whitespace,
    ForcedNewline,
    MarkdownHardLineBreak,
    MarkdownCodeSpanStart,
    MarkdownCodeSpanEnd,
    Literal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub value: String,
}

impl From<(Kind, &str)> for Token {
    fn from((kind, value): (Kind, &str)) -> Self {
        Token {
            kind,
            value: value.to_owned(),
        }
    }
}

const WORD: fn(char) -> bool = |c| c.is_ascii_alphanumeric() || c == '_';

const LETTER_OR_DIGIT: fn(char) -> bool = |c| {
    c == '_'
        || super::letter_table::LETTER_OR_DIGIT
            .binary_search_by(
                |&(low, high)| match (high < u32::from(c), low > u32::from(c)) {
                    (true, _) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => std::cmp::Ordering::Equal,
                },
            )
            .is_ok()
};

pub const SPACE: fn(char) -> bool = |c| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r');

#[derive(Debug, Clone, Copy)]
pub enum Flavor<'a> {
    Classic,
    Markdown(&'a super::markdown::Positions),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Pre,
    Code,
    Table,
    Snippet,
    Brace,
    InlineTag,
    MarkdownCode,
}

const PRESERVING: fn(&Context) -> bool = |open| {
    matches!(
        open,
        Context::Pre | Context::Code | Context::Table | Context::Snippet
    )
};

fn footer_tag(rest: &str) -> Option<usize> {
    let after = rest.strip_prefix('@')?;
    let param = after.strip_prefix("param").and_then(|tail| {
        let spaces = tail.len() - tail.trim_start_matches(SPACE).len();
        let name = tail[spaces..].strip_prefix('<')?;
        let length = name.len() - name.trim_start_matches(WORD).len();
        (spaces > 0 && length > 0 && name[length..].starts_with('>'))
            .then_some(1 + "param".len() + spaces + 1 + length + 1)
    });
    param.or_else(|| {
        after
            .starts_with(|c: char| c.is_ascii_lowercase())
            .then(|| 1 + after.len() - after.trim_start_matches(WORD).len())
    })
}

fn tag(rest: &str, opening: &str, names: &[&str]) -> Option<usize> {
    let after = rest.strip_prefix(opening)?;
    let name = names.iter().find(|name| {
        after
            .get(..name.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(name))
    })?;
    let tail = &after[name.len()..];
    let end = tail.find('>')?;
    (!tail.starts_with(WORD)).then_some(opening.len() + name.len() + end + 1)
}

struct State<'a> {
    tokens: Vec<Token>,
    stack: Vec<Context>,
    since_newline: bool,
    flavor: Flavor<'a>,
}

impl State<'_> {
    fn insert(&mut self, text: &str, position: usize) -> Option<usize> {
        let tokens = match self.flavor {
            Flavor::Classic => &[][..],
            Flavor::Markdown(positions) => &positions[position],
        };
        let consumed = tokens.iter().try_fold(0, |consumed, token| {
            let kind = match token.kind {
                Kind::MarkdownCodeSpanStart => {
                    self.stack.push(Context::MarkdownCode);
                    Kind::Literal
                }
                Kind::MarkdownCodeSpanEnd => {
                    let open = self
                        .stack
                        .iter()
                        .rposition(|open| *open == Context::MarkdownCode);
                    self.stack.truncate(open.unwrap_or(self.stack.len()));
                    Kind::Literal
                }
                kind => kind,
            };
            self.tokens.push(Token {
                kind,
                value: token.value.clone(),
            });
            text[position + consumed..]
                .starts_with(&token.value)
                .then_some(consumed + token.value.len())
        })?;
        match consumed {
            0 => Some(0),
            _ => self
                .insert(text, position + consumed)
                .map(|more| consumed + more),
        }
    }

    fn classify(&mut self, rest: &str) -> Option<(Kind, usize)> {
        let preserve = self.stack.iter().any(PRESERVING);
        let either = |preserved: Kind, otherwise: Kind| match preserve {
            true => preserved,
            false => otherwise,
        };
        let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        if rest[spaces..].starts_with('\n') {
            self.since_newline = false;
            return Some((either(Kind::ForcedNewline, Kind::Whitespace), spaces + 1));
        }
        if spaces > 0 {
            return Some((either(Kind::Literal, Kind::Whitespace), 1));
        }
        if self.stack.contains(&Context::MarkdownCode) {
            let first = rest.chars().next().map_or(0, char::len_utf8);
            let word = rest[first..].trim_start_matches(|c| !" \t\n`".contains(c));
            return Some((Kind::Literal, rest.len() - word.len()));
        }
        let escape = match self.flavor {
            Flavor::Classic => None,
            Flavor::Markdown(_) => rest.strip_prefix('\\').and_then(|tail| {
                let spaces = tail.len() - tail.trim_start_matches([' ', '\t']).len();
                match tail[spaces..].starts_with('\n') {
                    true => Some((Kind::MarkdownHardLineBreak, 1 + spaces + 1)),
                    false => tail
                        .chars()
                        .next()
                        .filter(|c| !"\n\r\u{85}\u{2028}\u{2029}".contains(*c))
                        .map(|c| (Kind::Literal, 1 + c.len_utf8())),
                }
            }),
        };
        if let Some((kind, length)) = escape {
            self.since_newline = kind == Kind::Literal;
            return Some((kind, length));
        }
        if let (false, Some(length)) = (self.since_newline, footer_tag(rest)) {
            self.since_newline = true;
            return self
                .stack
                .is_empty()
                .then_some((Kind::FooterTagStart, length));
        }
        self.since_newline = true;
        let braces = self
            .stack
            .iter()
            .any(|open| matches!(open, Context::InlineTag | Context::Brace | Context::Snippet));
        let first = rest.chars().next().map_or(0, char::len_utf8);
        let run = match self.flavor {
            Flavor::Classic => rest[first..].trim_start_matches(|c| !" \t\n@<{}*".contains(c)),
            Flavor::Markdown(_) => rest[first..].trim_start_matches(char::is_alphabetic),
        };
        let literal = (Kind::Literal, rest.len() - run.len());
        let snippet = rest
            .strip_prefix("{@snippet")
            .filter(|tail| !tail.starts_with(WORD));
        if snippet.is_some() {
            let (context, kind) = match braces {
                true => (Context::Brace, Kind::Literal),
                false => (Context::Snippet, Kind::SnippetBegin),
            };
            self.stack.push(context);
            return Some((kind, "{@snippet".len()));
        }
        if let Some(tail) = rest.strip_prefix("{@") {
            self.stack.push(Context::InlineTag);
            return Some((
                Kind::Literal,
                2 + tail.len() - tail.trim_start_matches(WORD).len(),
            ));
        }
        if rest.starts_with('{') {
            if braces {
                self.stack.push(Context::Brace);
            }
            return Some((Kind::Literal, 1));
        }
        if rest.starts_with('}') {
            let popped = self.stack.pop_if(|top| {
                matches!(top, Context::InlineTag | Context::Brace | Context::Snippet)
            });
            let kind = match popped {
                Some(Context::Snippet) => Kind::SnippetEnd,
                Some(_) | None => Kind::Literal,
            };
            return Some((kind, 1));
        }
        if self
            .stack
            .iter()
            .any(|open| matches!(open, Context::InlineTag | Context::Snippet))
        {
            return Some(literal);
        }
        let scoped = [
            ("pre", Context::Pre, Kind::PreOpen, Kind::PreClose),
            ("code", Context::Code, Kind::CodeOpen, Kind::CodeClose),
            ("table", Context::Table, Kind::TableOpen, Kind::TableClose),
        ];
        for (name, context, open, close) in scoped {
            if let Some(length) = tag(rest, "<", &[name]) {
                self.stack.push(context);
                return Some((either(Kind::Literal, open), length));
            }
            if let Some(length) = tag(rest, "</", &[name]) {
                if let Some(index) = self.stack.iter().rposition(|open| *open == context) {
                    self.stack.truncate(index);
                }
                let kind = match self.stack.iter().any(PRESERVING) {
                    true => Kind::Literal,
                    false => close,
                };
                return Some((kind, length));
            }
        }
        if preserve {
            return Some(literal);
        }
        let headers: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
        let plain: [(&str, &[&str], Kind); 11] = [
            ("<", &["p"], Kind::ParagraphOpen),
            ("</", &["p"], Kind::ParagraphClose),
            ("<", &["ul", "ol", "dl"], Kind::ListOpen),
            ("</", &["ul", "ol", "dl"], Kind::ListClose),
            ("<", &["li", "dt", "dd"], Kind::ListItemOpen),
            ("</", &["li", "dt", "dd"], Kind::ListItemClose),
            ("<", &["blockquote"], Kind::BlockQuoteOpen),
            ("</", &["blockquote"], Kind::BlockQuoteClose),
            ("<", headers, Kind::HeaderOpen),
            ("</", headers, Kind::HeaderClose),
            ("<", &["br"], Kind::Br),
        ];
        let moe = |marker: &str| {
            let after = rest.strip_prefix("<!--")?;
            let inner = after.trim_start_matches(SPACE).strip_prefix(marker)?;
            let tail = inner.trim_start_matches(SPACE).strip_prefix("-->")?;
            Some(rest.len() - tail.len())
        };
        let comment = rest
            .strip_prefix("<!--")
            .and_then(|after| after.find("-->"))
            .map(|end| "<!--".len() + end + "-->".len());
        plain
            .iter()
            .find_map(|(opening, names, kind)| {
                tag(rest, opening, names).map(|length| (*kind, length))
            })
            .or_else(|| moe("MOE:begin_intracomment_strip").map(|length| (Kind::MoeBegin, length)))
            .or_else(|| moe("MOE:end_intracomment_strip").map(|length| (Kind::MoeEnd, length)))
            .or_else(|| comment.map(|length| (Kind::HtmlComment, length)))
            .or(Some(literal))
    }
}

pub fn lex(text: &str, flavor: Flavor) -> Option<Vec<Token>> {
    let (begin, end) = match flavor {
        Flavor::Classic => ("/**", "*/"),
        Flavor::Markdown(_) => ("///", ""),
    };
    let mut state = State {
        tokens: vec![Token::from((Kind::BeginJavadoc, begin))],
        stack: Vec::new(),
        since_newline: false,
        flavor,
    };
    let mut rest = text;
    while !rest.is_empty() {
        rest = &rest[state.insert(text, text.len() - rest.len())?..];
        if rest.is_empty() {
            break;
        }
        let (kind, length) = state.classify(rest)?;
        state.tokens.push(Token::from((kind, &rest[..length])));
        rest = &rest[length..];
    }
    state.insert(text, text.len())?;
    if !state.stack.is_empty() {
        return None;
    }
    state.tokens.push(Token::from((Kind::EndJavadoc, end)));
    let joined = join_literals(state.tokens);
    let paragraphs = match flavor {
        Flavor::Classic => infer_paragraphs(joined),
        Flavor::Markdown(_) => joined,
    };
    let linked = optionalize_after_links(paragraphs);
    Some(deindent_pre_code(linked))
}

type Tokens = std::iter::Peekable<std::vec::IntoIter<Token>>;

fn literal_run(tokens: &mut Tokens) -> (String, String) {
    let mut accumulated = String::new();
    loop {
        let literals = std::iter::from_fn(|| tokens.next_if(|next| next.kind == Kind::Literal));
        accumulated.extend(literals.map(|literal| literal.value));
        let seen: String =
            std::iter::from_fn(|| tokens.next_if(|next| next.kind == Kind::Whitespace))
                .map(|whitespace| whitespace.value)
                .collect();
        let peeked = tokens.peek().filter(|next| next.kind == Kind::Literal);
        let at_sign = peeked.is_some_and(|next| next.value.starts_with('@'));
        let hyphenated = accumulated
            .strip_suffix('-')
            .and_then(|head| head.chars().last())
            .is_some_and(LETTER_OR_DIGIT);
        let joined = seen.matches('\n').count() == 1
            && hyphenated
            && peeked.is_some_and(|next| {
                next.value.starts_with(LETTER_OR_DIGIT) && next.value != "and" && next.value != "or"
            });
        match (at_sign, joined) {
            (true, _) => {
                accumulated.push(' ');
                accumulated.extend(tokens.next().map(|next| next.value));
            }
            (false, true) => accumulated.extend(tokens.next().map(|next| next.value)),
            (false, false) => return (accumulated, seen),
        }
    }
}

fn join_literals(input: Vec<Token>) -> Vec<Token> {
    let mut tokens = input.into_iter().peekable();
    let mut output: Vec<Token> = Vec::new();
    while let Some(next) = tokens.peek() {
        match next.kind {
            Kind::Literal => {
                let (literal, seen) = literal_run(&mut tokens);
                output.push(Token::from((Kind::Literal, literal.as_str())));
                if !seen.is_empty() {
                    output.push(Token::from((Kind::Whitespace, seen.as_str())));
                }
            }
            Kind::Whitespace => {
                let whitespace: String =
                    std::iter::from_fn(|| tokens.next_if(|next| next.kind == Kind::Whitespace))
                        .map(|whitespace| whitespace.value)
                        .collect();
                output.push(Token::from((Kind::Whitespace, whitespace.as_str())));
            }
            Kind::BeginJavadoc
            | Kind::EndJavadoc
            | Kind::FooterTagStart
            | Kind::ListOpen
            | Kind::ListClose
            | Kind::ListItemOpen
            | Kind::ListItemClose
            | Kind::ParagraphOpen
            | Kind::ParagraphClose
            | Kind::PreOpen
            | Kind::PreClose
            | Kind::CodeOpen
            | Kind::CodeClose
            | Kind::SnippetBegin
            | Kind::SnippetEnd
            | Kind::TableOpen
            | Kind::TableClose
            | Kind::BlockQuoteOpen
            | Kind::BlockQuoteClose
            | Kind::HeaderOpen
            | Kind::HeaderClose
            | Kind::Br
            | Kind::MoeBegin
            | Kind::MoeEnd
            | Kind::HtmlComment
            | Kind::OptionalLineBreak
            | Kind::MarkdownHardLineBreak
            | Kind::MarkdownCodeSpanStart
            | Kind::MarkdownCodeSpanEnd
            | Kind::ForcedNewline => output.extend(tokens.next()),
        }
    }
    output
}

fn infer_paragraphs(input: Vec<Token>) -> Vec<Token> {
    let mut tokens = input.into_iter().peekable();
    let mut output = Vec::new();
    while let Some(next) = tokens.next() {
        let literal = next.kind == Kind::Literal;
        output.push(next);
        let blank = tokens.peek().is_some_and(|after| {
            after.kind == Kind::Whitespace && after.value.matches('\n').count() > 1
        });
        if literal && blank {
            output.extend(tokens.next());
            if tokens
                .peek()
                .is_some_and(|after| after.kind == Kind::Literal)
            {
                output.push(Token::from((Kind::ParagraphOpen, "<p>")));
            }
        }
    }
    output
}

fn optionalize_after_links(input: Vec<Token>) -> Vec<Token> {
    let mut tokens = input.into_iter().peekable();
    let mut output = Vec::new();
    while let Some(next) = tokens.next() {
        let link = next.kind == Kind::Literal
            && next
                .value
                .strip_prefix("href=")
                .and_then(|tail| tail.strip_suffix('>'))
                .is_some_and(|inner| !inner.contains('>'));
        output.push(next);
        let whitespace = link
            .then(|| tokens.next_if(|after| after.kind == Kind::Whitespace))
            .flatten();
        output.extend(
            whitespace.map(|whitespace| {
                Token::from((Kind::OptionalLineBreak, whitespace.value.as_str()))
            }),
        );
    }
    output
}

fn deindent_pre_code(input: Vec<Token>) -> Vec<Token> {
    let mut tokens = input.into_iter().peekable();
    let mut output = Vec::new();
    while let Some(next) = tokens.next() {
        let pre = next.kind == Kind::PreOpen;
        output.push(next);
        if !pre {
            continue;
        }
        let newlines: Vec<Token> =
            std::iter::from_fn(|| tokens.next_if(|after| after.kind == Kind::ForcedNewline))
                .collect();
        let code = tokens.next_if(|after| {
            after.kind == Kind::Literal && after.value.trim_start_matches([' ', '\t']) == "{@code"
        });
        match code {
            None => {
                output.extend(newlines);
                output.extend(tokens.next());
            }
            Some(head) => {
                let block: Vec<Token> =
                    std::iter::from_fn(|| tokens.next_if(|after| after.kind != Kind::PreClose))
                        .collect();
                output.extend(deindent_block(&head, &block));
            }
        }
    }
    output
}

fn deindent_block(head: &Token, rest: &[Token]) -> Vec<Token> {
    let opening = Token::from((Kind::Literal, head.value.trim_matches(|c: char| c <= ' ')));
    let first = rest
        .iter()
        .position(|token| token.kind != Kind::ForcedNewline)
        .unwrap_or(rest.len());
    let last = rest
        .iter()
        .rposition(|token| token.kind != Kind::ForcedNewline)
        .map_or(first, |index| index + 1);
    let body = &rest[first..last];
    let (tail, init) = body.split_last().unwrap_or((head, &[]));
    let brace = tail.kind == Kind::Literal && tail.value.ends_with('}');
    let saved: Vec<Token> = match (brace, tail.value.len() > 1) {
        (true, true) => init
            .iter()
            .cloned()
            .chain([
                Token::from((Kind::Literal, &tail.value[..tail.value.len() - 1])),
                Token::from((Kind::ForcedNewline, "")),
            ])
            .collect(),
        (true, false) => init.to_vec(),
        (false, _) => body.to_vec(),
    };
    let trim = saved
        .iter()
        .filter(|token| token.kind == Kind::Literal)
        .filter_map(|token| token.value.find(|c| c != ' '))
        .min()
        .unwrap_or(0);
    let shifted = saved.into_iter().map(|token| match token.kind {
        Kind::Literal if trim > 0 && token.value.len() > trim => Token {
            value: token.value[trim..].to_owned(),
            ..token
        },
        _ => token,
    });
    let closing = match brace {
        true => Token::from((Kind::Literal, "}")),
        false => Token::from((Kind::ForcedNewline, "\n")),
    };
    [opening, Token::from((Kind::ForcedNewline, "\n"))]
        .into_iter()
        .chain(shifted)
        .chain([closing])
        .collect()
}
