# Reject the markdown Javadoc that MarkdownPositions' checks throw on

Bounded context: `gjf` (`src/gjf/javadoc/`, `src/gjf.rs`), with the `LINE_UNDEFINED` rejection position of task 157.

GJF throws VerifyException, and Spotless rejects the file at `LINE_UNDEFINED`, where jeer still defers the file:
- Task 149: a markdown comment holding U+0085, U+2028 or U+2029, since GJF's lexer throws when one starts a word in a code span (WORD_IN_CODE_SPAN_PATTERN starts with Java's `.`, which does not match them)
- Task 150: a list item whose marker is not where MarkdownPositions looks for it (its start plus the marker indent, counted in columns, which a tab shifts), or is followed by something other than whitespace or the end, where its `verify(matcher.lookingAt())` throws

jeer formats a comment in its rewrite, which cannot reject yet. The task is sorted after task 157, which adds the `LINE_UNDEFINED` rejection position it needs.

## Done when
- jeer rejects those files with GJF's message at `LINE_UNDEFINED` exactly where GJF throws, and formats the comments with these characters or markers where GJF does
- `parse` defers nothing: it returns `Option` no more, and `gjf::step` no longer defers an input because of a `///` comment
- Small tests check both sides against the oracle
