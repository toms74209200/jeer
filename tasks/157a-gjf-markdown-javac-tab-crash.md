# Reject the markdown Javadoc that javac's doc comment parser crashes on

Bounded context: `gjf` (`src/gjf/javadoc/`, `src/gjf.rs`), with the `LINE_UNDEFINED` rejection position of task 157.

GJF's RemoveUnusedImports scans the doc comment of each declaration with javac's doc comment parser (`trees.getDocCommentTree`; a `///` comment on a local variable is not scanned). javac's Markdown.update (DocCommentParser, JDK 25), run at the start of each line, throws StringIndexOutOfBoundsException when a list marker opens on a line: it sets `peekIndex = listItemIndent - indent`, where `listItemIndent` counts the columns after the marker with tab stops, and then calls `line.substring(peekIndex)` on the line's characters. A tab after the marker can push `peekIndex` past the line's end, as in `/// -\ta` (`Range [4, 3) out of bounds for length 3`), while `/// -\tab` and a nested `-\tb` on a continuation line stay in range. Spotless then rejects the file at `LINE_UNDEFINED` with `google-java-format(java.lang.StringIndexOutOfBoundsException) Range [a, b) out of bounds for length b`. Task 150 defers every bullet marker followed by a tab until then.

The task is sorted after task 157, which adds the `LINE_UNDEFINED` rejection position it needs.

## Done when
- jeer runs Markdown.update's container and list marker logic over the lines of each declaration's `///` comment as javac reads them (its pointer, LineKind patterns in their order, the start of each block tag's content), and rejects the file with GJF's message at `LINE_UNDEFINED` exactly when javac throws
- The deferral of a bullet marker followed by a tab is gone
- Small tests check both sides of the condition against the oracle
