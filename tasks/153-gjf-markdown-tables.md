# Port GFM tables for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

The GFM TablesExtension's TableBlockParser on the block parser of task 148, with MarkdownPositions' `MarkdownTable` over the table's source span and `JavadocWriter.writeMarkdownTable`.

## Done when
- `parse` no longer defers a `|`, and the `javadoc-markdown-table` fixture passes through `gjf::format`, not only through the step's deferral
