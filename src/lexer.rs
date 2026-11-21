mod identifier_table;
mod scan;
pub mod token;
mod unicode_escapes;

use token::{LexError, Token};

pub fn scan(source: &str) -> Result<Vec<Token>, LexError> {
    let text = unicode_escapes::translate(source)?;
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < text.chars.len() {
        let (kind, end) = scan::next_token(&text.chars, index).map_err(|kind| LexError {
            kind,
            offset: text.offsets[index],
        })?;
        tokens.push(Token {
            kind,
            start: text.offsets[index],
            end: text.offsets[end],
        });
        index = end;
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::token::{Keyword, LexErrorKind, Punct, TokenKind};
    use super::*;

    #[test]
    fn when_scan_with_one_token_then_returns_its_kind_over_the_whole_source() {
        let cases = [
            (" \t\u{c}", TokenKind::Whitespace),
            ("\n", TokenKind::Newline),
            ("\r\n", TokenKind::Newline),
            ("\r", TokenKind::Newline),
            ("// line", TokenKind::LineComment),
            ("/* block */", TokenKind::BlockComment),
            ("/**/", TokenKind::BlockComment),
            ("/** doc */", TokenKind::JavadocComment),
            ("/***/", TokenKind::JavadocComment),
            ("name$_1", TokenKind::Identifier),
            ("var", TokenKind::Identifier),
            ("record", TokenKind::Identifier),
            ("yield", TokenKind::Identifier),
            ("_x", TokenKind::Identifier),
            ("名前", TokenKind::Identifier),
            ("αβγ", TokenKind::Identifier),
            ("€uro", TokenKind::Identifier),
            ("e\u{301}", TokenKind::Identifier),
            ("a\u{200b}b", TokenKind::Identifier),
            ("x\u{7f}", TokenKind::Identifier),
            ("𝑥", TokenKind::Identifier),
            ("class", TokenKind::Keyword(Keyword::Class)),
            ("_", TokenKind::Keyword(Keyword::Underscore)),
            ("true", TokenKind::True),
            ("false", TokenKind::False),
            ("null", TokenKind::Null),
            ("0", TokenKind::IntegerLiteral),
            ("1_000L", TokenKind::IntegerLiteral),
            ("0x1F_ffl", TokenKind::IntegerLiteral),
            ("0b10_1", TokenKind::IntegerLiteral),
            ("0_7", TokenKind::IntegerLiteral),
            ("1.5", TokenKind::FloatingLiteral),
            ("1.", TokenKind::FloatingLiteral),
            (".5e-3", TokenKind::FloatingLiteral),
            ("1e+10d", TokenKind::FloatingLiteral),
            ("3f", TokenKind::FloatingLiteral),
            ("0x1.8p1", TokenKind::FloatingLiteral),
            ("0x.8P-2f", TokenKind::FloatingLiteral),
            ("'a'", TokenKind::CharLiteral),
            ("'\\n'", TokenKind::CharLiteral),
            ("'\\377'", TokenKind::CharLiteral),
            ("'\\7'", TokenKind::CharLiteral),
            ("'\\47'", TokenKind::CharLiteral),
            ("'é'", TokenKind::CharLiteral),
            ("'\\u0041'", TokenKind::CharLiteral),
            ("\"s \\\" \\t \\0 \\12 \\s\"", TokenKind::StringLiteral),
            (
                "\"\"\"\n  text \"\" \\\"\"\" \\\n  \"\"\"",
                TokenKind::TextBlock,
            ),
            ("\"\"\"  \r\n  text\r\n\"\"\"", TokenKind::TextBlock),
            ("\"\"\"\r  a \\\r\"\"\"", TokenKind::TextBlock),
            ("\"\"\"\r\n  a \\\r\n\"\"\"", TokenKind::TextBlock),
            (">>>=", TokenKind::Punct(Punct::GtGtGtEq)),
            ("->", TokenKind::Punct(Punct::Arrow)),
            ("::", TokenKind::Punct(Punct::ColonColon)),
            ("...", TokenKind::Punct(Punct::Ellipsis)),
            ("@", TokenKind::Punct(Punct::At)),
            ("/", TokenKind::Punct(Punct::Slash)),
            ("/=", TokenKind::Punct(Punct::SlashEq)),
            ("\u{1a}", TokenKind::Whitespace),
        ];
        for (source, kind) in cases {
            let tokens = scan(source);

            assert_eq!(
                tokens,
                Ok(vec![Token {
                    kind,
                    start: 0,
                    end: source.len()
                }]),
                "{source:?}"
            );
        }
    }

    #[test]
    fn when_scan_with_each_keyword_or_punct_then_returns_its_kind() {
        let cases = [
            ("abstract", TokenKind::Keyword(Keyword::Abstract)),
            ("assert", TokenKind::Keyword(Keyword::Assert)),
            ("boolean", TokenKind::Keyword(Keyword::Boolean)),
            ("break", TokenKind::Keyword(Keyword::Break)),
            ("byte", TokenKind::Keyword(Keyword::Byte)),
            ("case", TokenKind::Keyword(Keyword::Case)),
            ("catch", TokenKind::Keyword(Keyword::Catch)),
            ("char", TokenKind::Keyword(Keyword::Char)),
            ("class", TokenKind::Keyword(Keyword::Class)),
            ("const", TokenKind::Keyword(Keyword::Const)),
            ("continue", TokenKind::Keyword(Keyword::Continue)),
            ("default", TokenKind::Keyword(Keyword::Default)),
            ("do", TokenKind::Keyword(Keyword::Do)),
            ("double", TokenKind::Keyword(Keyword::Double)),
            ("else", TokenKind::Keyword(Keyword::Else)),
            ("enum", TokenKind::Keyword(Keyword::Enum)),
            ("extends", TokenKind::Keyword(Keyword::Extends)),
            ("final", TokenKind::Keyword(Keyword::Final)),
            ("finally", TokenKind::Keyword(Keyword::Finally)),
            ("float", TokenKind::Keyword(Keyword::Float)),
            ("for", TokenKind::Keyword(Keyword::For)),
            ("goto", TokenKind::Keyword(Keyword::Goto)),
            ("if", TokenKind::Keyword(Keyword::If)),
            ("implements", TokenKind::Keyword(Keyword::Implements)),
            ("import", TokenKind::Keyword(Keyword::Import)),
            ("instanceof", TokenKind::Keyword(Keyword::Instanceof)),
            ("int", TokenKind::Keyword(Keyword::Int)),
            ("interface", TokenKind::Keyword(Keyword::Interface)),
            ("long", TokenKind::Keyword(Keyword::Long)),
            ("native", TokenKind::Keyword(Keyword::Native)),
            ("new", TokenKind::Keyword(Keyword::New)),
            ("package", TokenKind::Keyword(Keyword::Package)),
            ("private", TokenKind::Keyword(Keyword::Private)),
            ("protected", TokenKind::Keyword(Keyword::Protected)),
            ("public", TokenKind::Keyword(Keyword::Public)),
            ("return", TokenKind::Keyword(Keyword::Return)),
            ("short", TokenKind::Keyword(Keyword::Short)),
            ("static", TokenKind::Keyword(Keyword::Static)),
            ("strictfp", TokenKind::Keyword(Keyword::Strictfp)),
            ("super", TokenKind::Keyword(Keyword::Super)),
            ("switch", TokenKind::Keyword(Keyword::Switch)),
            ("synchronized", TokenKind::Keyword(Keyword::Synchronized)),
            ("this", TokenKind::Keyword(Keyword::This)),
            ("throw", TokenKind::Keyword(Keyword::Throw)),
            ("throws", TokenKind::Keyword(Keyword::Throws)),
            ("transient", TokenKind::Keyword(Keyword::Transient)),
            ("try", TokenKind::Keyword(Keyword::Try)),
            ("void", TokenKind::Keyword(Keyword::Void)),
            ("volatile", TokenKind::Keyword(Keyword::Volatile)),
            ("while", TokenKind::Keyword(Keyword::While)),
            ("(", TokenKind::Punct(Punct::LParen)),
            (")", TokenKind::Punct(Punct::RParen)),
            ("{", TokenKind::Punct(Punct::LBrace)),
            ("}", TokenKind::Punct(Punct::RBrace)),
            ("[", TokenKind::Punct(Punct::LBracket)),
            ("]", TokenKind::Punct(Punct::RBracket)),
            (";", TokenKind::Punct(Punct::Semicolon)),
            (",", TokenKind::Punct(Punct::Comma)),
            (".", TokenKind::Punct(Punct::Dot)),
            ("...", TokenKind::Punct(Punct::Ellipsis)),
            ("@", TokenKind::Punct(Punct::At)),
            ("::", TokenKind::Punct(Punct::ColonColon)),
            ("=", TokenKind::Punct(Punct::Assign)),
            (">", TokenKind::Punct(Punct::Gt)),
            ("<", TokenKind::Punct(Punct::Lt)),
            ("!", TokenKind::Punct(Punct::Bang)),
            ("~", TokenKind::Punct(Punct::Tilde)),
            ("?", TokenKind::Punct(Punct::Question)),
            (":", TokenKind::Punct(Punct::Colon)),
            ("->", TokenKind::Punct(Punct::Arrow)),
            ("==", TokenKind::Punct(Punct::EqEq)),
            (">=", TokenKind::Punct(Punct::GtEq)),
            ("<=", TokenKind::Punct(Punct::LtEq)),
            ("!=", TokenKind::Punct(Punct::BangEq)),
            ("&&", TokenKind::Punct(Punct::AmpAmp)),
            ("||", TokenKind::Punct(Punct::PipePipe)),
            ("++", TokenKind::Punct(Punct::PlusPlus)),
            ("--", TokenKind::Punct(Punct::MinusMinus)),
            ("+", TokenKind::Punct(Punct::Plus)),
            ("-", TokenKind::Punct(Punct::Minus)),
            ("*", TokenKind::Punct(Punct::Star)),
            ("/", TokenKind::Punct(Punct::Slash)),
            ("&", TokenKind::Punct(Punct::Amp)),
            ("|", TokenKind::Punct(Punct::Pipe)),
            ("^", TokenKind::Punct(Punct::Caret)),
            ("%", TokenKind::Punct(Punct::Percent)),
            ("<<", TokenKind::Punct(Punct::LtLt)),
            (">>", TokenKind::Punct(Punct::GtGt)),
            (">>>", TokenKind::Punct(Punct::GtGtGt)),
            ("+=", TokenKind::Punct(Punct::PlusEq)),
            ("-=", TokenKind::Punct(Punct::MinusEq)),
            ("*=", TokenKind::Punct(Punct::StarEq)),
            ("/=", TokenKind::Punct(Punct::SlashEq)),
            ("&=", TokenKind::Punct(Punct::AmpEq)),
            ("|=", TokenKind::Punct(Punct::PipeEq)),
            ("^=", TokenKind::Punct(Punct::CaretEq)),
            ("%=", TokenKind::Punct(Punct::PercentEq)),
            ("<<=", TokenKind::Punct(Punct::LtLtEq)),
            (">>=", TokenKind::Punct(Punct::GtGtEq)),
            (">>>=", TokenKind::Punct(Punct::GtGtGtEq)),
        ];
        for (source, kind) in cases {
            let tokens = scan(source);

            assert_eq!(
                tokens,
                Ok(vec![Token {
                    kind,
                    start: 0,
                    end: source.len()
                }]),
                "{source:?}"
            );
        }
    }

    #[test]
    fn when_scan_with_any_source_then_returns_tokens_that_tile_it() {
        let sources = [
            "package a.b;\n\nimport java.util.*;\r\n",
            "class A<T extends List<List<String>>> { int x = a >> 2 >>> b; }",
            "/** doc */ @Override public void f() { return; } // end",
            "var s = \"\"\"\n    hi\n    \"\"\";\nchar c = '\\'';",
            "x->x::y; a ? b : c; i++ + --j; a.b.c(1.0f, 0x1p3, 'q');",
            "\\u0063lass \\u0041 {}",
            "switch (o) { case Integer i when i > 0 -> yield i; default -> {} }",
        ];
        for source in sources {
            let tokens = scan(source).unwrap_or_default();

            let joined: String = tokens.iter().map(|t| &source[t.start..t.end]).collect();
            let contiguous = tokens.windows(2).all(|w| w[0].end == w[1].start);
            assert_eq!((joined.as_str(), contiguous), (source, true), "{source:?}");
        }
    }

    #[test]
    fn when_scan_with_shift_like_brackets_then_returns_greedy_puncts() {
        let source = "a>>=b>>c>=d";

        let kinds: Vec<TokenKind> = scan(source)
            .unwrap_or_default()
            .iter()
            .map(|t| t.kind)
            .collect();

        assert_eq!(
            kinds,
            [
                TokenKind::Identifier,
                TokenKind::Punct(Punct::GtGtEq),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::GtGt),
                TokenKind::Identifier,
                TokenKind::Punct(Punct::GtEq),
                TokenKind::Identifier,
            ]
        );
    }

    #[test]
    fn when_scan_with_unicode_escapes_then_returns_tokens_over_the_escaped_source() {
        let source = "\\u0069nt x\\u003b";

        let tokens = scan(source);

        assert_eq!(
            tokens,
            Ok(vec![
                Token {
                    kind: TokenKind::Keyword(Keyword::Int),
                    start: 0,
                    end: 8
                },
                Token {
                    kind: TokenKind::Whitespace,
                    start: 8,
                    end: 9
                },
                Token {
                    kind: TokenKind::Identifier,
                    start: 9,
                    end: 10
                },
                Token {
                    kind: TokenKind::Punct(Punct::Semicolon),
                    start: 10,
                    end: 16
                },
            ])
        );
    }

    #[test]
    fn when_scan_with_a_lexical_error_then_returns_its_kind_and_offset() {
        let cases = [
            ("a /* open", LexErrorKind::UnterminatedComment, 2),
            ("/*/", LexErrorKind::UnterminatedComment, 0),
            ("x = \"open", LexErrorKind::UnterminatedString, 4),
            ("\"line\nbreak\"", LexErrorKind::UnterminatedString, 0),
            ("'a", LexErrorKind::UnterminatedChar, 0),
            ("'ab'", LexErrorKind::UnterminatedChar, 0),
            ("'", LexErrorKind::UnterminatedChar, 0),
            ("'\n'", LexErrorKind::UnterminatedChar, 0),
            ("'😀'", LexErrorKind::UnterminatedChar, 0),
            ("'\\uD83D\\uDE00'", LexErrorKind::UnterminatedChar, 0),
            ("''", LexErrorKind::EmptyChar, 0),
            ("\"\\q\"", LexErrorKind::IllegalEscape, 0),
            ("'\\\n'", LexErrorKind::IllegalEscape, 0),
            ("\"\"\"\nopen", LexErrorKind::UnterminatedTextBlock, 0),
            ("\"\"\" x\"\"\"", LexErrorKind::IllegalTextBlockOpen, 0),
            ("\"\"\"\n\\x\"\"\"", LexErrorKind::IllegalEscape, 0),
            ("a # b", LexErrorKind::IllegalCharacter, 2),
            ("•", LexErrorKind::IllegalCharacter, 0),
            ("a → b", LexErrorKind::IllegalCharacter, 2),
            ("\u{301}a", LexErrorKind::IllegalCharacter, 0),
            ("\u{3000}", LexErrorKind::IllegalCharacter, 0),
            ("\\u12", LexErrorKind::IllegalUnicodeEscape, 0),
            ("1_", LexErrorKind::IllegalUnderscore, 0),
            ("0x_1", LexErrorKind::IllegalUnderscore, 0),
            ("1._5", LexErrorKind::IllegalUnderscore, 0),
            ("0x", LexErrorKind::MalformedNumber, 0),
            ("0x1.8", LexErrorKind::MalformedNumber, 0),
            ("1e", LexErrorKind::MalformedNumber, 0),
            ("1e+", LexErrorKind::MalformedNumber, 0),
        ];
        for (source, kind, offset) in cases {
            let error = scan(source).err();

            assert_eq!(error, Some(LexError { kind, offset }), "{source:?}");
        }
    }

    #[test]
    fn when_display_with_each_error_kind_then_returns_javac_like_message() {
        let cases = [
            (LexErrorKind::UnterminatedComment, "unclosed comment"),
            (LexErrorKind::UnterminatedString, "unclosed string literal"),
            (LexErrorKind::UnterminatedChar, "unclosed character literal"),
            (LexErrorKind::UnterminatedTextBlock, "unclosed text block"),
            (LexErrorKind::EmptyChar, "empty character literal"),
            (LexErrorKind::IllegalEscape, "illegal escape character"),
            (LexErrorKind::IllegalUnicodeEscape, "illegal unicode escape"),
            (
                LexErrorKind::IllegalTextBlockOpen,
                "illegal text block open delimiter sequence, missing line terminator",
            ),
            (LexErrorKind::IllegalCharacter, "illegal character"),
            (LexErrorKind::IllegalUnderscore, "illegal underscore"),
            (LexErrorKind::MalformedNumber, "malformed number"),
        ];
        for (kind, message) in cases {
            let shown = kind.to_string();

            assert_eq!(shown, message);
        }
    }
}
