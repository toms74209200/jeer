# Format simple Javadoc as google-java-format does

Bounded context: `gjf` (`src/gjf/`, with the Javadoc formatter in `src/gjf/javadoc/`).

Port JavadocFormatter's lexer and writer for text, paragraphs, blank lines and block tags; stop deferring `/**` comments; port LayoutBuilder's Javadoc branches (the forced break after a Javadoc before a token, no blank line requested after it, a trailing Javadoc broken onto its own line) and `hasJavaDoc`'s effect on blank lines.

## Done when
- `javadoc-simple`, `javadoc-empty`, `javadoc-one-line-tag`, `javadoc-field`, `javadoc-paragraphs`, `javadoc-blank-lines`, `javadoc-params` and `javadoc-wrap` pass with their `skip` markers deleted
- `gjf::step` no longer defers inputs with `/**` comments
