# Format new expressions as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitNewClass` with anonymous class bodies and `visitNewArray` with dimensions and initializers.

## Done when
- A `new` expression with a block lambda argument at the head of a chain with dereferences after it is not hugged, as doNotHug does for the chain head (task 130 left that case to this task)
- A `new` with a class body does not hug its only block lambda argument, as doNotHug does in visitNewClass (task 130 left that case to this task too)
- `expr-new-array`, `stmt-switch-arrow` and `stmt-assert-throw` pass with their `skip` markers deleted (the last two are a rule switch, task 123, and an assert and a throw, task 124, that use `new`)
- Small tests or fixtures cover `new` with arguments, with type arguments, with a class body, and qualified `outer.new Inner()`
