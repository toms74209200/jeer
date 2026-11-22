# Format assert, throw, synchronized and local classes as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitAssert`, `visitThrow`, `visitSynchronized` and local class declarations in blocks.

## Done when
- `stmt-assert-throw`, `stmt-synchronized` and `stmt-local-class` pass with their `skip` markers deleted
