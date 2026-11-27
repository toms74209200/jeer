# Port block quotes for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

BlockQuoteParser on the block parser of task 148, with MarkdownPositions' `MarkdownBlockQuoteOpen` (the `> ?` it consumes) and `MarkdownBlockQuoteClose`, the lexer's `BLOCKQUOTE` context and `BlockQuoteMarker` at the start of a line, the lexer consuming a position token's non-empty text, and the writer's block-quote indent, which also puts `>` on blank lines inside the quote.

## Done when
- `parse` no longer defers lines starting with `>`, and the `javadoc-markdown-blockquote` fixture passes with its `skip` marker deleted
