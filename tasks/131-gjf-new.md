# Format new expressions as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitNewClass` with anonymous class bodies and `visitNewArray` with dimensions and initializers.

## Done when
- `expr-new-array` and `stmt-switch-arrow` pass with their `skip` markers deleted (the latter is a rule switch, task 123, whose default throws `new IllegalStateException()`)
- Small tests or fixtures cover `new` with arguments, with type arguments, with a class body, and qualified `outer.new Inner()`
