# Add fixtures for new expressions

Bounded context: `tests/fixtures/` (no `src` change), a spec change for task 131-gjf-new.

Task 131's Done-when asks for `new` with arguments, with type arguments, with a class body and qualified `outer.new Inner()`, and for the two doNotHug cases of `new`; no fixture has them.

## Done when
- An oracle fixture in `tests/fixtures/gjf/` covers `new` with arguments, with type arguments, with a class body, a qualified `outer.new Inner()`, a block lambda argument of a `new` at the head of a chain with a dereference after it, and a block lambda argument of a `new` with a class body; it carries a `skip` marker until task 131
