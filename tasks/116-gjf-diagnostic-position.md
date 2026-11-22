# Report google-java-format's errors at its position

LayoutBuilder.diagnostic reports a FormattingError at inputPosition, the start of the last node the visitor synced (`sync(node)`), not at the token that failed to match. jeer reports an "expected token" rejection at the next input token, so the line can differ (`int\n      \u0061b;`: the oracle reports line 2, jeer line 3).

## Done when
- The visitor records the start position of each node it syncs as JavaInputAstVisitor does, and rejections report that position
- Small tests cover rejections whose failing token is on a later line than the node's start
