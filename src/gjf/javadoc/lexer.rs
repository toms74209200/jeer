use crate::gjf::Unformatted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BeginJavadoc,
    EndJavadoc,
    FooterTagStart,
    ParagraphOpen,
    ParagraphClose,
    Whitespace,
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

fn paragraph_tag(rest: &str) -> Option<(Kind, usize)> {
    let (kind, opening) = match rest.starts_with("</") {
        true => (Kind::ParagraphClose, 2),
        false => (Kind::ParagraphOpen, 1),
    };
    let tail = rest.get(opening..)?;
    let bounded = tail.starts_with(['p', 'P']) && !tail[1..].starts_with(WORD);
    let end = tail.find('>')?;
    bounded.then_some((kind, opening + end + 1))
}

pub fn lex(text: &str) -> Result<Vec<Token>, Unformatted> {
    let mut tokens = vec![Token::from((Kind::BeginJavadoc, "/**"))];
    let mut rest = text;
    while !rest.is_empty() {
        let since_newline = tokens
            .iter()
            .rev()
            .find(|token| token.kind != Kind::Whitespace || token.value.contains('\n'))
            .is_some_and(|token| !matches!(token.kind, Kind::Whitespace | Kind::BeginJavadoc));
        let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let footer = footer_tag(rest).filter(|_| !since_newline);
        let (kind, length) = match (rest[spaces..].starts_with('\n'), spaces > 0, footer) {
            (true, _, _) => (Kind::Whitespace, spaces + 1),
            (false, true, _) => (Kind::Whitespace, 1),
            (false, false, Some(length)) => (Kind::FooterTagStart, length),
            (false, false, None) => {
                match (paragraph_tag(rest), rest.starts_with(['<', '{', '}'])) {
                    (Some(tag), _) => tag,
                    (None, true) => return Err(Unformatted::Deferred),
                    (None, false) => {
                        let first = rest.chars().next().map_or(0, char::len_utf8);
                        let run = rest[first..].trim_start_matches(|c| !" \t\n@<{}*".contains(c));
                        (Kind::Literal, rest.len() - run.len())
                    }
                }
            }
        };
        tokens.push(Token::from((kind, &rest[..length])));
        rest = &rest[length..];
    }
    tokens.push(Token::from((Kind::EndJavadoc, "*/")));
    let joined = join_literals(tokens);
    Ok(infer_paragraphs(joined))
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
            | Kind::ParagraphOpen
            | Kind::ParagraphClose => output.extend(tokens.next()),
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
