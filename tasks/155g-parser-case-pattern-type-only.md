# Accept a case label's parenthesized type as javac does

Bounded context: `parser` (`src/parser/statements.rs`).

javac accepts `case R(String` then `) -> {}` (a case label it reads as an expression), and GJF formats it; jeer reads a record pattern and rejects it with `<identifier> expected`. Found in task 155's review.

## Done when
- Oracle fixtures, added in a fixtures task first, cover a case label of a name applied to a type without a binding, and pass
