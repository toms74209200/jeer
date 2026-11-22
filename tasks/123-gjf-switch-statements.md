# Format switch statements as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitSwitch` and `visitCase` for colon groups and `->` rules in statements, with comments inside them.

## Done when
- `stmt-switch-old`, `stmt-switch-arrow`, `comment-in-switch` and `string-switch-case` pass with their `skip` markers deleted
