# Port fenced code blocks for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

FencedCodeBlockParser on the block parser of task 148, with MarkdownPositions' `MarkdownFencedCodeBlock` (the opening fence and info, the closing fence or the opening length when there is none, the literal) and `JavadocWriter.writeMarkdownFencedCodeBlock`.

## Done when
- `parse` no longer defers lines starting with `` ` `` or `~` fences, and the `javadoc-markdown-fenced-code` fixture passes with its `skip` marker deleted
