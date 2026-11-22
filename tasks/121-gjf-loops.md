# Format loops and labels as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitForLoop`, `visitEnhancedForLoop`, `visitDoWhileLoop`, `visitLabeledStatement`, `visitBreak` and `visitContinue`; `visitWhileLoop` came with task 120, since `stmt-empty-blocks` needs it.

## Done when
- `stmt-loops` and `stmt-labels` pass with their `skip` markers deleted
