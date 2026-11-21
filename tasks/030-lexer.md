# Add a lossless Java lexer

The lexer of design doc §7: every token of Java 25 (JLS §3), with comments, whitespace and newlines kept as trivia; `\uXXXX` escapes handled; text blocks; contextual keywords left as identifiers for the parser.

## Done when
- Small tests cover every token kind, unicode escapes, text blocks and the lexical errors javac reports (unterminated comment, string or char literal, illegal character)
- `scripts/check-roundtrip.sh dev` (added in this task) passes: for every dev corpus file the concatenated token texts equal the input byte for byte
