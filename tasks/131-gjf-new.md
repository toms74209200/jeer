# Format new expressions as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitNewClass` with anonymous class bodies and `visitNewArray` with dimensions and initializers.

## Done when
- `expr-new-array`, `stmt-switch-arrow` and `stmt-assert-throw` pass with their `skip` markers deleted (the last two are a rule switch, task 123, and an assert and a throw, task 124, that use `new`)
- Small tests or fixtures cover `new` with arguments, with type arguments, with a class body, and qualified `outer.new Inner()`
