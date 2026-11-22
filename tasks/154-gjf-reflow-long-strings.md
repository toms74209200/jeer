# Reflow long strings as google-java-format does

Bounded context: `gjf` (`src/gjf/`), with an oracle profile added in `oracle/`.

Design doc §2 and §4: a project whose Spotless configuration turns on `googleJavaFormat().reflowLongStrings()` must switch with zero diff, so jeer ports StringWrapper, which the Spotless glue runs after ImportOrderer when the option is on. The pinned oracle profile `gjf` runs the step's defaults (reflow off); a profile with reflow on judges this task. Selecting the option from a project's configuration is task 170.

## Done when
- `oracle/build.gradle.kts` has a profile running `googleJavaFormat("2.0.0").reflowLongStrings()`, and spec-change fixtures for it (long strings in arguments, assignments, returns and concatenations, and strings it leaves alone) are generated with `scripts/generate-fixture.sh` and carry `skip` markers
- jeer has that profile, it runs StringWrapper after the gjf pipeline, and the fixtures pass with their markers deleted
