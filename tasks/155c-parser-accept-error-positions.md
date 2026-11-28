# Report the errors javac's accept() reports where javac does

Bounded context: `parser` (`src/parser.rs`, `src/parser/`).

javac reports an expected token through JavacParser.accept or ident() at the end of the previous token, and Spotless prints that line. Task 155 moved `<identifier> expected` there; these errors are still at the next token in jeer, a line later across a newline or a comment, and some with another message. Found in task 155's review, oracle against jeer:
- A case label without `:` or `->` (`case 1` then `x();` on the next line, the same for `default` and in switch expressions): `L4 ... : or -> expected` against `5:7: ':' or '->' expected`
- A module body holding something other than a directive (`module m {` then `1`): `L1 ... '}' expected` against `2:3: module directive expected`
- A missing type after `throws` or `new` (`void f() throws` then `{}`, `a = new` then `;`): `L2 ... <identifier> expected` against `3:3: <type> expected`
- An enum body that ends after a constant (`enum E {` then `A` and the end of the file): `L2 ... reached end of file while parsing` against `3:1: <identifier> expected`
- A try resource with modifiers and no name (`try (final R` then `)`): `L1 ... <identifier> expected` against `1:27: illegal start of expression`

## Done when
- Oracle fixtures, added in a fixtures task first, cover each case above across a newline and a comment, and pass
- The parser reports each with javac's message at javac's line
