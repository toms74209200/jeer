# Format markdown Javadoc as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`///` comments: GJF treats one at a declaration as markdown Javadoc whatever its length (markdownJavadocPositions), formats it with JavadocFormatter, and breaks a trailing one onto its own line.

## Done when
- `javadoc-markdown-bare` and `javadoc-markdown-short-trailing` pass with their `skip` markers deleted
- `gjf::format` no longer defers inputs with `///` comments
