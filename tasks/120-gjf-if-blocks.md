# Format if statements and blocks as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitIf`, `visitStatement` with its collapse and blank-line options, and the statement headers (`HeaderMark`) that the solver ported in task 105 but nothing constructs yet.

## Done when
- `stmt-if-else`, `stmt-empty-blocks` and `comment-before-else` pass with their `skip` markers deleted
- The visitor builds `Doc::Header`, and `HeaderMark` is no longer covered by the `#[expect(dead_code)]` on `mod gjf`
