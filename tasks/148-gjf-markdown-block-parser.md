# Port commonmark-java's block parsing for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

Task 147 formats markdown Javadoc made of paragraphs only: `javadoc::markdown::parse` splits the comment text at blank lines and returns `None`, which defers the file, for anything else (#74 decided to port commonmark-java 0.29.0 rather than take a dependency). This task ports DocumentParser's line loop as GJF's parser runs it (open block parsers, continuation and lazy continuation lines, tab columns, source spans of `BLOCKS_AND_INLINES`) with the block starts that are not containers: ATX and setext headings, thematic breaks, indented code blocks, HTML blocks and link reference definitions. MarkdownPositions turns an ATX heading into `HeaderOpen`/`HeaderClose` and throws `UnsupportedOperationException` on a setext heading, a thematic break, an indented code block and a link reference definition, which GJF reports as a lex failure: the comment comes back unchanged.

## Done when
- `parse` no longer defers lines starting with `#`, `=`, `_`, `[`, an indentation of four columns or a tab, and returns the positions or the lex failure GJF reaches for them
- Oracle fixtures, added in a fixtures task first, cover an ATX heading and each construct GJF leaves unchanged, and pass
