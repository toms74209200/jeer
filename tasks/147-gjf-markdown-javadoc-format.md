# Format markdown Javadoc at a declaration as google-java-format does

Bounded context: `gjf` (`src/gjf/javadoc/`).

A `///` comment longer than four characters at a declaration is markdown Javadoc, which GJF formats with JavadocFormatter's markdown path: markdownCommentText, MarkdownPositions (a CommonMark parse), the lexer's markdown tokens and the writer's markdown methods. A CommonMark parser is not a dependency jeer may take on without asking (docs/development.md), so decide how to parse first: port the part MarkdownPositions uses or ask.

## Done when
- Oracle fixtures, added in a fixtures task first, cover markdown Javadoc with text, paragraphs, lists, code spans, fenced code, tables and block quotes, and pass with their `skip` markers deleted
- `gjf::step` no longer defers inputs with `///` comments
