# Reject empty type arguments in a type as javac does

Bounded context: `parser` (`src/parser/types.rs`).

javac rejects empty type arguments in a declared type with `illegal start of type`; jeer accepts them. Found in task 155's review: `class A {` then `List<` then `> x;` then `}` is rejected by the oracle at `L3 ... illegal start of type` and accepted by jeer.

## Done when
- Oracle fixtures, added in a fixtures task first, cover empty type arguments in a field, a parameter and a local variable type, and pass with javac's message and line
