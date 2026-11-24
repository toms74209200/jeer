# Format the rest of classic Javadoc markup as google-java-format does

Bounded context: `gjf` (`src/gjf/javadoc/`).

Port the JavadocLexer and JavadocWriter branches that task 141 leaves: a literal `<`, snippets, tables, blockquotes, headers, `<br>` (standardizeBrToken), MOE strip comments, HTML comments and optionalizeSpacesAfterLinks.

## Done when
- The fixtures of task 143 pass with their `skip` markers deleted
- `gjf::step` no longer defers inputs with `/**` comments
