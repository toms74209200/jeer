# Reject a modifier on an instanceof type test as javac does

Bounded context: `parser` (`src/parser/expressions.rs`).

javac rejects a modifier on an `instanceof` type that has no binding, with `modifier final not allowed here`; jeer accepts it. Found in task 155's review: `return o instanceof final String` then `;` on the next line is rejected by the oracle at L3, and jeer formats it.

## Done when
- Oracle fixtures, added in a fixtures task first, cover `final` on an `instanceof` type without a binding, and pass with javac's message and line
