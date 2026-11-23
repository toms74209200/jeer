# Format an anonymous class at the head of a chain as google-java-format does

Bounded context: `gjf` (`src/gjf/`), with its fixture in a fixtures task first.

GJF's visitDot opens a `new` with a class body at the head of a chain at ZERO and puts the chain's first dot right after its `}`, instead of breaking and indenting the rest +4 (`new Object() { ... }.toString()`). The visitor of task 131 defers that case.

## Done when
- An oracle fixture covers an anonymous class at the head of a chain with one and with several dereferences, added by a fixtures task first
- The visitor formats the chain head as visitDot does, and the fixture passes with its `skip` marker deleted
