# Add fixtures for google-java-format's fixed point

Bounded context: `tests/fixtures/` (no `src` change), a spec change for task 117-pipeline-padded-cell.

## Done when
- Oracle fixtures in `tests/fixtures/gjf/` cover trailing `//` comments that GJF wraps after a field, a statement and an initializer, where one pass of GJF is not its fixed point; each carries a `skip` marker
