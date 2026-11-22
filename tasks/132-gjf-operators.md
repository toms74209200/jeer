# Format casts, instanceof, conditionals and method chains as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitTypeCast`, `visitInstanceOf`, `visitConditionalExpression`, and the remaining dotted-chain cases.

## Done when
- `expr-casts`, `expr-instanceof`, `expr-ternary` and `expr-method-chain` pass with their `skip` markers deleted
