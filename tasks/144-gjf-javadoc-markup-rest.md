# Format the rest of classic Javadoc markup as google-java-format does

Bounded context: `gjf` (`src/gjf/javadoc/`).

Port the JavadocLexer and JavadocWriter branches that task 141 leaves: literal `<`, `{` and `}`, inline tags and snippets with their brace nesting, tables, blockquotes, headers, `<br>` (standardizeBrToken), MOE strip comments, HTML comments, optionalizeSpacesAfterLinks, and checkMatchingTags returning the comment unchanged.

## Done when
- The fixtures of task 143 pass with their `skip` markers deleted
- `gjf::step` no longer defers inputs with `/**` comments
