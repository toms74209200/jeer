# Port lists for markdown Javadoc

Bounded context: `gjf` (`src/gjf/javadoc/`).

ListBlockParser and ListItemParser on the block parser of task 148, with the container part of DocumentParser's line loop that task 148 left out since only top-level leaf blocks existed before (open block parsers matched in turn, continuation and lazy continuation lines, unmatched blocks closed, source spans across containers), with MarkdownPositions' list tokens (`ListOpen`/`ListClose` with an empty value, `ListItemOpen` with the marker matched by `LIST_ITEM_START_PATTERN` at the marker indent, the first paragraph of an item not visited) and the writer's markdown list handling (no indent for an empty list open, no blank lines requested around it).

## Done when
- `parse` no longer defers lines starting with `-`, `+`, `*` or a digit, and the `javadoc-markdown-lists` fixture passes with its `skip` marker deleted
