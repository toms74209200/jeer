# Format markdown Javadoc holding U+0000

Bounded context: `gjf` (`src/gjf/javadoc/`).

Task 147 defers a markdown comment holding U+0000, which GJF formats. commonmark-java replaces it with U+FFFD when it reads a line (DocumentParser.prepareLine) and keeps every position, since both are one UTF-16 unit, so the block and inline rules see U+FFFD: it is not a control character for a link destination (LinkScanner), not blank for a link label, and not trimmed from a fence's info string. GJF's lexer reads the original text, so the tokens whose text MarkdownPositions takes from the input (MarkdownFencedCodeBlock, MarkdownTable) hold U+0000, while the fence's info and literal, which come from commonmark-java's nodes, hold U+FFFD, and JavadocWriter.writeMarkdownFencedCodeBlock writes them. In jeer's UTF-8 offsets U+FFFD takes three bytes where U+0000 takes one. Task 154 ported the `<` deferral of task 147 and split this one off.

## Done when
- `parse` no longer defers a U+0000
- The fixture `javadoc-markdown-nul` passes
