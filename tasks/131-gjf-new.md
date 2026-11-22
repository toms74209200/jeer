# Format new expressions as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitNewClass` with anonymous class bodies and `visitNewArray` with dimensions and initializers.

## Done when
- `expr-new-array` passes with its `skip` marker deleted
- Small tests or fixtures cover `new` with arguments, with type arguments, with a class body, and qualified `outer.new Inner()`
