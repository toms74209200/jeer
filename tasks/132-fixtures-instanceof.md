# Add a fixture for instanceof without patterns

Bounded context: `tests/fixtures/` and `tasks/` (no `src` change), a fixtures task for task 132-gjf-operators.

`expr-instanceof` uses a type pattern and a record pattern, which are task 152's constructs, so task 132 cannot pass it.

## Done when
- Task 152's Done-when names `expr-instanceof`, which keeps its `skip` marker, and task 132's no longer does
- An oracle fixture `expr-instanceof-type` covers `instanceof` with a type, its negation in parentheses, and an `instanceof` too long for one line, with a `skip` marker until task 132
