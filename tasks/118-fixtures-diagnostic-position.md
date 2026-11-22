# Add fixtures for google-java-format's error positions

Bounded context: `tests/fixtures/` (no `src` change), a spec change for task 118-gjf-diagnostic-position.

## Done when
- Oracle-rejected fixtures in `tests/fixtures/gjf/` cover an error whose failing token is on a later line than its node's start, and an error after a cleanup that changed the text before it; each carries a `skip` marker
