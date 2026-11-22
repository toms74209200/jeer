# Add fixtures for the simple statements without new or records

Bounded context: `tests/fixtures/` and the gjf small test (no visitor change).

Task 124 ported visitAssert, visitThrow and visitSynchronized (whose only fixture never wraps its header) and relies on scan for local class declarations, but its fixtures `stmt-assert-throw` and `stmt-local-class` use `new` and a local record, so they wait for tasks 131 and 151 and nothing in the tree guards these statements meanwhile.

## Done when
- Oracle fixtures in `tests/fixtures/gjf/` cover a bare assert, an assert whose condition and detail wrap, a throw without `new` (such as `throw e;`), a synchronized header whose lock expression wraps past 100 columns, and a local class in a method body without a record or `new` (such as `class L { int x; }`)
- They pass without `skip` markers and join the gjf small test
