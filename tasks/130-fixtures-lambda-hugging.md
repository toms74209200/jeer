# Add fixtures for lambda hugging

Bounded context: `tests/fixtures/` (no `src` change), a spec change for task 130-gjf-lambdas.

Task 130's fixtures cover block and expression lambdas, typed parameters, plain method references and an enum constant with a class body. The hugging it ports also decides how a lone lambda argument sits in a call on the first line of a chain, in a call with trailing dereferences, in a prefixed chain, and next to other arguments, and it covers varargs lambda parameters and method references with type arguments; no fixture has these.

## Done when
- Oracle fixtures in `tests/fixtures/gjf/` cover a hugged block lambda argument, a block lambda in a call followed by a dereference, lambdas in a stream chain, an expression lambda that wraps, a lambda after another argument, two chained calls with block lambdas, a varargs lambda parameter, a method reference with type arguments, and a lambda argument after a line comment; each carries a `skip` marker until task 130
