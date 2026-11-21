# Match Java's identifier characters in the lexer

The lexer accepts any non-ASCII, non-whitespace character as an identifier character, an approximation of `Character.isJavaIdentifierStart` / `isJavaIdentifierPart`. Input that javac rejects for an illegal character in an identifier (such as `•` or `→` outside strings and comments) is accepted.

## Done when
- A spec-change commit adds fixtures of our own under `tests/fixtures/gjf/` whose input uses an illegal non-ASCII character outside strings and comments, with `expected.error` from the oracle, and others using legal ones (letters of several scripts, currency symbols, combining marks), with `skip` markers
- The lexer classifies characters by Java's rules (Unicode general categories as `Character` defines them, from a table generated from the Unicode data the pinned JDK uses), with small tests
- Those fixtures pass once the `gjf` profile rejects and accepts input (task 060); until then the small tests show the classification
