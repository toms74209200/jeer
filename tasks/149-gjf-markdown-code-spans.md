# Port code spans for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

MarkdownPositions marks each `Code` node of commonmark-java's inline parse with `MarkdownCodeSpanStart`/`MarkdownCodeSpanEnd`, and the lexer lexes inside them in `MARKDOWN_CODE_CONTEXT`, turning the backticks into literals so no line breaks next to them. Where code spans are depends on the inline parser's other openers that can take a backtick first: backslash escapes, autolinks and inline HTML (InlineParserImpl, BackticksInlineParser, BackslashInlineParser, AutolinkInlineParser, HtmlInlineParser), with inline source spans.

## Done when
- `parse` no longer defers a backtick, and the `javadoc-markdown-code-span` fixture passes with its `skip` marker deleted
