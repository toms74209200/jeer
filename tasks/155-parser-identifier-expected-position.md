# Report `<identifier> expected` where javac does

Bounded context: `parser` (`src/parser.rs`, `src/parser/`).

javac reports `<identifier> expected` at the end of the token before the one it could not take (JavacParser.reportSyntaxError with the previous token's end, as jeer's `Parser::missing` does), and Spotless prints that line. jeer's `expect_identifier` reports it at the start of the next token (`Parser::fail`), so the line differs whenever a newline or a comment separates the two. Found in task 154b: a fence info in markdown Javadoc holding an entity that decodes to a line terminator (`&NewLine;`, `&#10;`, `&#x0A;`, `&#0000010;`, `&#13;`) puts a bare word on a line of its own in GJF's output, and its reparse fails; the oracle reports `L5 ... <identifier> expected` for each. This task fixes the parser half; where the gjf step reports a rejection of its own output is task 155a's. The parser half alone shows on the formatted text given directly:

```java
class B {
  /// Text.
  ///
  /// ```a
b
  /// x
  /// ```
  void f() {}
}
```

The oracle rejects it at `L5 ... <identifier> expected`, and jeer at 8:3.

## Done when
- Oracle fixtures, added in a fixtures task first, cover `<identifier> expected` after a newline and after a comment, and pass
- Every `<identifier> expected` the parser reports is at the position javac reports it, except at a lambda parameter and a case label's record pattern, where javac first decides what it parses with JavacParser.analyzeParens and analyzePattern (task 155h)
