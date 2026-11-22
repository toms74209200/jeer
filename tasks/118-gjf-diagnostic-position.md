# Report google-java-format's errors at its position

Bounded context: `gjf` (`src/gjf/`).

LayoutBuilder.diagnostic reports a FormattingError at inputPosition, the start of the last node the visitor synced (`sync(node)`), not at the token that failed to match. jeer reports an "expected token" rejection at the next input token, so the line can differ (`int\n      \u0061b;`: the oracle reports line 2, jeer line 3).

Rejections after a cleanup (task 112) carry offsets in the cleaned text, while `pipeline` locates them in the input it was given; GJF reports them against the cleaned contents too, so the line can differ from the input's.

## Done when
- The visitor records the start position of each node it syncs as JavaInputAstVisitor does, and rejections report that position
- A rejection after a cleanup that changed the text reports the oracle's line and column, through the line-and-column `Rejection` of §7.1, so `pipeline` is unchanged
- `reject-position-later-line` and `reject-position-after-cleanup` (task 118-fixtures-diagnostic-position) pass with their `skip` markers deleted, with a small test that checks the line of every oracle-rejected fixture against the `L<line>` of its `expected.error`, and small tests cover rejections whose failing token is on a later line than the node's start, and rejections in inputs that cleanups changed
