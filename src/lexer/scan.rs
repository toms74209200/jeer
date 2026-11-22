use super::identifier::{self, CharClass};
use super::token::{LexErrorKind, PUNCTS, TokenKind, WORDS};

type Scanned = Result<(TokenKind, usize), LexErrorKind>;

pub fn next_token(chars: &[char], start: usize) -> Scanned {
    let at = |i: usize| chars.get(i).copied();
    let identifier_start = |c: char| identifier::class(c) == CharClass::Start;
    let identifier_part = |c: char| identifier::class(c) != CharClass::Other;
    match chars[start] {
        ' ' | '\t' | '\u{c}' => {
            let end = (start..chars.len())
                .find(|&i| !matches!(chars[i], ' ' | '\t' | '\u{c}'))
                .unwrap_or(chars.len());
            Ok((TokenKind::Whitespace, end))
        }
        '\n' => Ok((TokenKind::Newline, start + 1)),
        '\r' if at(start + 1) == Some('\n') => Ok((TokenKind::Newline, start + 2)),
        '\r' => Ok((TokenKind::Newline, start + 1)),
        '\u{1a}' if start + 1 == chars.len() => Ok((TokenKind::Whitespace, start + 1)),
        '/' if at(start + 1) == Some('/') => {
            let end = (start..chars.len())
                .find(|&i| matches!(chars[i], '\n' | '\r'))
                .unwrap_or(chars.len());
            Ok((TokenKind::LineComment, end))
        }
        '/' if at(start + 1) == Some('*') => {
            let close = (start + 2..chars.len().saturating_sub(1))
                .find(|&i| chars[i] == '*' && chars[i + 1] == '/')
                .ok_or(LexErrorKind::UnterminatedComment)?;
            let javadoc = at(start + 2) == Some('*') && close > start + 2;
            let kind = if javadoc {
                TokenKind::JavadocComment
            } else {
                TokenKind::BlockComment
            };
            Ok((kind, close + 2))
        }
        '"' if at(start + 1) == Some('"') && at(start + 2) == Some('"') => text_block(chars, start),
        '"' => string(chars, start),
        '\'' if at(start + 1) == Some('\'') => Err(LexErrorKind::EmptyChar),
        '\'' => {
            let content_end = match at(start + 1) {
                None | Some('\n' | '\r') => Err(LexErrorKind::UnterminatedChar),
                Some('\\') => escape(chars, start + 1),
                Some(c) if c.len_utf16() == 2 => Err(LexErrorKind::UnterminatedChar),
                Some(_) => Ok(start + 2),
            }?;
            match at(content_end) {
                Some('\'') => Ok((TokenKind::CharLiteral, content_end + 1)),
                _ => Err(LexErrorKind::UnterminatedChar),
            }
        }
        '0'..='9' => number(chars, start),
        '.' if at(start + 1).is_some_and(|c| c.is_ascii_digit()) => number(chars, start),
        c if identifier_start(c) => {
            let end = (start + 1..chars.len())
                .find(|&i| !identifier_part(chars[i]))
                .unwrap_or(chars.len());
            let word: String = chars[start..end].iter().collect();
            let kind = WORDS
                .iter()
                .find(|(text, _)| *text == word)
                .map_or(TokenKind::Identifier, |(_, kind)| *kind);
            Ok((kind, end))
        }
        _ => PUNCTS
            .iter()
            .find(|(text, _)| {
                text.chars()
                    .enumerate()
                    .all(|(k, c)| at(start + k) == Some(c))
            })
            .map(|(text, punct)| (TokenKind::Punct(*punct), start + text.len()))
            .ok_or(LexErrorKind::IllegalCharacter),
    }
}

pub fn escape_value(chars: &[char], backslash: usize) -> (char, usize) {
    let at = |i: usize| chars.get(i).copied();
    match at(backslash + 1) {
        Some('b') => ('\u{8}', backslash + 2),
        Some('s') => (' ', backslash + 2),
        Some('t') => ('\t', backslash + 2),
        Some('n') => ('\n', backslash + 2),
        Some('f') => ('\u{c}', backslash + 2),
        Some('r') => ('\r', backslash + 2),
        Some(first @ '0'..='7') => {
            let longest = if first <= '3' { 3 } else { 2 };
            let digits: Vec<u8> = chars[backslash + 1..]
                .iter()
                .take(longest)
                .map_while(|c| c.to_digit(8).and_then(|digit| u8::try_from(digit).ok()))
                .collect();
            let value = digits.iter().fold(0u8, |value, digit| value * 8 + digit);
            (char::from(value), backslash + 1 + digits.len())
        }
        Some(other) => (other, backslash + 2),
        None => ('\\', backslash + 1),
    }
}

fn escape(chars: &[char], backslash: usize) -> Result<usize, LexErrorKind> {
    match chars.get(backslash + 1) {
        Some('b' | 's' | 't' | 'n' | 'f' | 'r' | '"' | '\'' | '\\' | '0'..='7') => {
            Ok(escape_value(chars, backslash).1)
        }
        _ => Err(LexErrorKind::IllegalEscape),
    }
}

fn string(chars: &[char], start: usize) -> Scanned {
    let mut i = start + 1;
    loop {
        match chars.get(i) {
            None | Some('\n' | '\r') => return Err(LexErrorKind::UnterminatedString),
            Some('\\') => i = escape(chars, i)?,
            Some('"') => return Ok((TokenKind::StringLiteral, i + 1)),
            Some(_) => i += 1,
        }
    }
}

fn text_block(chars: &[char], start: usize) -> Scanned {
    let at = |i: usize| chars.get(i).copied();
    let mut i = start + 3;
    while matches!(at(i), Some(' ' | '\t' | '\u{c}')) {
        i += 1;
    }
    if !matches!(at(i), Some('\n' | '\r')) {
        return Err(LexErrorKind::IllegalTextBlockOpen);
    }
    loop {
        match (at(i), at(i + 1), at(i + 2)) {
            (None, _, _) => return Err(LexErrorKind::UnterminatedTextBlock),
            (Some('"'), Some('"'), Some('"')) => return Ok((TokenKind::TextBlock, i + 3)),
            (Some('\\'), Some('\r'), Some('\n')) => i += 3,
            (Some('\\'), Some('\n' | '\r'), _) => i += 2,
            (Some('\\'), _, _) => i = escape(chars, i)?,
            (Some(_), _, _) => i += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Radix {
    Binary,
    Decimal,
    Hex,
}

impl Radix {
    fn digit(self, c: char) -> bool {
        match self {
            Radix::Binary => c == '0' || c == '1',
            Radix::Decimal => c.is_ascii_digit(),
            Radix::Hex => c.is_ascii_hexdigit(),
        }
    }
}

enum Run {
    Empty,
    Digits(usize),
}

fn digits(chars: &[char], start: usize, radix: Radix) -> Result<Run, LexErrorKind> {
    let end = (start..chars.len())
        .find(|&i| !(radix.digit(chars[i]) || chars[i] == '_'))
        .unwrap_or(chars.len());
    match (
        end > start,
        chars.get(start),
        chars.get(end.wrapping_sub(1)),
    ) {
        (false, _, _) => Ok(Run::Empty),
        (true, Some('_'), _) | (true, _, Some('_')) => Err(LexErrorKind::IllegalUnderscore),
        (true, _, _) => Ok(Run::Digits(end)),
    }
}

fn number(chars: &[char], start: usize) -> Scanned {
    let at = |i: usize| chars.get(i).copied();
    let (radix, body) = match (at(start), at(start + 1)) {
        (Some('0'), Some('x' | 'X')) => (Radix::Hex, start + 2),
        (Some('0'), Some('b' | 'B')) => (Radix::Binary, start + 2),
        _ => (Radix::Decimal, start),
    };
    let integer = digits(chars, body, radix)?;
    let integer_end = match integer {
        Run::Digits(end) => end,
        Run::Empty => body,
    };
    let fraction = match (radix, at(integer_end)) {
        (Radix::Decimal | Radix::Hex, Some('.')) => Some(digits(chars, integer_end + 1, radix)?),
        (Radix::Binary, _) | (Radix::Decimal | Radix::Hex, _) => None,
    };
    let mantissa_end = match (&integer, &fraction) {
        (Run::Empty, None | Some(Run::Empty)) => return Err(LexErrorKind::MalformedNumber),
        (_, Some(Run::Digits(end))) => *end,
        (_, Some(Run::Empty)) => integer_end + 1,
        (Run::Digits(end), None) => *end,
    };
    let exponent = match (radix, at(mantissa_end)) {
        (Radix::Hex, Some('p' | 'P')) | (Radix::Decimal, Some('e' | 'E')) => {
            let sign = usize::from(matches!(at(mantissa_end + 1), Some('+' | '-')));
            match digits(chars, mantissa_end + 1 + sign, Radix::Decimal)? {
                Run::Digits(end) => Some(end),
                Run::Empty => return Err(LexErrorKind::MalformedNumber),
            }
        }
        (Radix::Binary | Radix::Decimal | Radix::Hex, _) => None,
    };
    let end = exponent.unwrap_or(mantissa_end);
    let floating = fraction.is_some() || exponent.is_some();
    match (radix, floating, exponent, at(end)) {
        (Radix::Hex, true, None, _) => Err(LexErrorKind::MalformedNumber),
        (Radix::Decimal | Radix::Hex, _, _, Some('f' | 'F' | 'd' | 'D')) => {
            Ok((TokenKind::FloatingLiteral, end + 1))
        }
        (_, false, _, Some('l' | 'L')) => Ok((TokenKind::IntegerLiteral, end + 1)),
        (_, true, _, _) => Ok((TokenKind::FloatingLiteral, end)),
        (_, false, _, _) => Ok((TokenKind::IntegerLiteral, end)),
    }
}
