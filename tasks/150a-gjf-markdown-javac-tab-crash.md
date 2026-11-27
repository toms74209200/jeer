# Reject the markdown Javadoc that javac's doc comment parser crashes on

Bounded context: `gjf` (`src/gjf/javadoc/`, `src/gjf.rs`).

GJF's RemoveUnusedImports scans Javadoc with javac's doc comment parser, and javac's markdown parsing (DocCommentParser$Markdown.update, initMarkdownLine) throws StringIndexOutOfBoundsException on some lines: a bullet marker followed by a tab and a single character (`/// -\ta`, `/// +\tb`) does, while `/// -\tab` and `/// 1.\ta` do not. Spotless then reports the file as rejected (`google-java-format(java.lang.StringIndexOutOfBoundsException)`). Task 150 defers every bullet marker followed by a tab, so jeer never formats what GJF rejects.

## Done when
- The condition under which javac throws is found from its source and reproduced: jeer rejects those files with GJF's message, and formats the other bullet markers followed by a tab
- Small tests check both sides of the condition against the oracle
