# Format switch expressions and patterns as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

Switch expressions, `yield`, `case` patterns with `when`, record patterns, unnamed variables and patterns, and `var`.

## Done when
- `modern-switch-expression`, `modern-yield`, `modern-record-pattern`, `modern-enhanced-for-pattern`, `modern-unnamed` and `modern-var` and `expr-instanceof` (an `instanceof` with a type pattern and a record pattern) pass with their `skip` markers deleted
