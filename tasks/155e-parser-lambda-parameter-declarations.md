# Reject the lambda parameter lists javac rejects

Bounded context: `parser` (`src/parser/expressions.rs`).

javac checks a lambda's parameter list as a whole and reports `invalid lambda parameter declaration` when it mixes forms; jeer accepts some of these or reports another message. Found in task 155's review, oracle against jeer:
- `Object o = (String` then `, int b) -> 1;`: the oracle rejects it at L2, jeer accepts it
- `Object o = (int a, int` then `) -> 1;`: the oracle reports `invalid lambda parameter declaration` at L2, jeer `<identifier> expected` on line 2

## Done when
- Oracle fixtures, added in a fixtures task first, cover mixed and incomplete lambda parameter lists, and pass with javac's message and line
