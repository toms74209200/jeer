use crate::gjf::Unformatted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BeginJavadoc,
    EndJavadoc,
    FooterTagStart,
    ListOpen,
    ListClose,
    ListItemOpen,
    ListItemClose,
    ParagraphOpen,
    ParagraphClose,
    PreOpen,
    PreClose,
    CodeOpen,
    CodeClose,
    Whitespace,
    ForcedNewline,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Pre,
    Code,
    Brace,
    InlineTag,
}

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

struct State {
    tokens: Vec<Token>,
    stack: Vec<Context>,
    since_newline: bool,
}

impl State {
    fn classify(&mut self, rest: &str) -> Result<Option<(Kind, usize)>, Unformatted> {
        let preserve = self
            .stack
            .iter()
            .any(|open| matches!(open, Context::Pre | Context::Code));
        let either = |preserved: Kind, otherwise: Kind| match preserve {
            true => preserved,
            false => otherwise,
        };
        let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        if rest[spaces..].starts_with('\n') {
            self.since_newline = false;
            return Ok(Some((
                either(Kind::ForcedNewline, Kind::Whitespace),
                spaces + 1,
            )));
        }
        if spaces > 0 {
            return Ok(Some((either(Kind::Literal, Kind::Whitespace), 1)));
        }
        if let (false, Some(length)) = (self.since_newline, footer_tag(rest)) {
            self.since_newline = true;
            return Ok(self
                .stack
                .is_empty()
                .then_some((Kind::FooterTagStart, length)));
        }
        self.since_newline = true;
        let braces = self
            .stack
            .iter()
            .any(|open| matches!(open, Context::InlineTag | Context::Brace));
        let first = rest.chars().next().map_or(0, char::len_utf8);
        let run = rest[first..].trim_start_matches(|c| !" \t\n@<{}*".contains(c));
        let literal = (Kind::Literal, rest.len() - run.len());
        if rest
            .strip_prefix("{@snippet")
            .is_some_and(|tail| !tail.starts_with(WORD))
        {
            return Err(Unformatted::Deferred);
        }
        if let Some(tail) = rest.strip_prefix("{@") {
            self.stack.push(Context::InlineTag);
            return Ok(Some((
                Kind::Literal,
                2 + tail.len() - tail.trim_start_matches(WORD).len(),
            )));
        }
        if rest.starts_with('{') {
            if braces {
                self.stack.push(Context::Brace);
            }
            return Ok(Some((Kind::Literal, 1)));
        }
        if rest.starts_with('}') {
            self.stack
                .pop_if(|top| matches!(top, Context::InlineTag | Context::Brace));
            return Ok(Some((Kind::Literal, 1)));
        }
        if self.stack.contains(&Context::InlineTag) {
            return Ok(Some(literal));
        }
        let scoped = [
            ("pre", Context::Pre, Kind::PreOpen, Kind::PreClose),
            ("code", Context::Code, Kind::CodeOpen, Kind::CodeClose),
        ];
        for (name, context, open, close) in scoped {
            if let Some(length) = tag(rest, "<", &[name]) {
                self.stack.push(context);
                return Ok(Some((either(Kind::Literal, open), length)));
            }
            if let Some(length) = tag(rest, "</", &[name]) {
                if let Some(index) = self.stack.iter().rposition(|open| *open == context) {
                    self.stack.truncate(index);
                }
                let preserving = self
                    .stack
                    .iter()
                    .any(|open| matches!(open, Context::Pre | Context::Code));
                let kind = match preserving {
                    true => Kind::Literal,
                    false => close,
                };
                return Ok(Some((kind, length)));
            }
        }
        if preserve {
            return Ok(Some(literal));
        }
        let plain: [(&str, &[&str], Kind); 6] = [
            ("<", &["p"], Kind::ParagraphOpen),
            ("</", &["p"], Kind::ParagraphClose),
            ("<", &["ul", "ol", "dl"], Kind::ListOpen),
            ("</", &["ul", "ol", "dl"], Kind::ListClose),
            ("<", &["li", "dt", "dd"], Kind::ListItemOpen),
            ("</", &["li", "dt", "dd"], Kind::ListItemClose),
        ];
        let matched = plain.iter().find_map(|(opening, names, kind)| {
            tag(rest, opening, names).map(|length| (*kind, length))
        });
        match (matched, rest.starts_with('<')) {
            (Some(matched), _) => Ok(Some(matched)),
            (None, true) => Err(Unformatted::Deferred),
            (None, false) => Ok(Some(literal)),
        }
    }
}

pub fn lex(text: &str) -> Result<Option<Vec<Token>>, Unformatted> {
    let mut state = State {
        tokens: vec![Token::from((Kind::BeginJavadoc, "/**"))],
        stack: Vec::new(),
        since_newline: false,
    };
    let mut rest = text;
    while !rest.is_empty() {
        let Some((kind, length)) = state.classify(rest)? else {
            return Ok(None);
        };
        state.tokens.push(Token::from((kind, &rest[..length])));
        rest = &rest[length..];
    }
    if !state.stack.is_empty() {
        return Ok(None);
    }
    state.tokens.push(Token::from((Kind::EndJavadoc, "*/")));
    let joined = join_literals(state.tokens);
    let paragraphs = infer_paragraphs(joined);
    Ok(Some(deindent_pre_code(paragraphs)))
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
