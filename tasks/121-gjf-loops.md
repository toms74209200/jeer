# Format loops and labels as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitForLoop`, `visitEnhancedForLoop`, `visitWhileLoop`, `visitDoWhileLoop`, `visitLabeledStatement`, `visitBreak` and `visitContinue`.

## Done when
- `stmt-loops` and `stmt-labels` pass with their `skip` markers deleted
