# Add fixtures for long-string reflow

Bounded context: `oracle/` and `tests/fixtures/` (no `src` change), a spec change for tasks 154-gjf-reflow-long-strings and 155.

## Done when
- `oracle/build.gradle.kts` has a profile `gjf-reflow` running `googleJavaFormat("2.0.0").reflowLongStrings()`
- Fixtures in `tests/fixtures/gjf-reflow/` cover long strings in arguments, assignments, returns and concatenations, and strings it leaves alone; they are generated with `scripts/generate-fixture.sh` and carry `skip` markers
