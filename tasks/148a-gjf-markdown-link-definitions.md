# Port link reference definitions for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

commonmark-java takes link reference definitions from the start of a paragraph when it closes (LinkReferenceDefinitionParser with LinkScanner), and MarkdownPositions throws `UnsupportedOperationException` on one, which GJF reports as a lex failure: the comment comes back unchanged. Task 148 still defers a paragraph whose first line starts with `[`. A paragraph that starts with `[` but holds no definition stays a paragraph.

## Done when
- `parse` no longer defers a paragraph starting with `[`, and returns `Unsupported` exactly when commonmark-java finds a definition
- The `javadoc-markdown-link-definition` fixture passes through `gjf::format`, not only through the step's deferral
