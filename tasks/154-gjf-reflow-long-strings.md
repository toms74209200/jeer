# Reflow long strings as google-java-format does

Bounded context: `gjf` (`src/gjf/`); the oracle profile in `oracle/` and its fixtures come in a spec-change commit first.

Design doc §2 and §4: a project whose Spotless configuration turns on `googleJavaFormat().reflowLongStrings()` must switch with zero diff, so jeer ports StringWrapper, which Spotless's glue runs after ImportOrderer when the option is on. The pinned oracle profile `gjf` runs the step's defaults (reflow off); a profile with reflow on judges this task, and task 155 makes it runnable from the CLI.

## Done when
- `oracle/build.gradle.kts` has a profile `gjf-reflow` running `googleJavaFormat("2.0.0").reflowLongStrings()`, and fixtures for it in `tests/fixtures/gjf-reflow/` (long strings in arguments, assignments, returns and concatenations, and strings it leaves alone) are generated with `scripts/generate-fixture.sh` and carry `skip` markers
- `gjf::step` takes options with `reflow_long_strings`, runs StringWrapper last when it is on, and a small test in `gjf` loads every `gjf-reflow` fixture and gets its expected output
