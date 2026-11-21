# Add fixtures for inputs the oracle rejects

A spec-change task: no production code.

## Done when
- `tests/fixtures/gjf/` has at least 10 fixtures of our own whose input GJF rejects (unbalanced braces, a missing semicolon, a malformed generic, a broken lambda, an unterminated text block, a `yield` outside a switch expression, ...), each with `expected.error` from `scripts/generate-fixture.sh` and a `skip` marker
- `scripts/check-fixtures-oracle.sh` passes
