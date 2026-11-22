# Reflow long strings as google-java-format does

Bounded context: `gjf` (`src/gjf/`), judged by the fixtures of task 154-fixtures-reflow.

Design doc §2 and §4: a project whose Spotless configuration turns on `googleJavaFormat().reflowLongStrings()` must switch with zero diff, so jeer ports StringWrapper, which Spotless's glue runs after ImportOrderer when the option is on. The pinned oracle profile `gjf` runs the step's defaults (reflow off); a profile with reflow on judges this task, and task 155 makes it runnable from the CLI.

## Done when
- `gjf::Options` gains `reflow_long_strings` (off by default), `gjf::step` runs StringWrapper last when it is on, and a small test in `gjf` loads every `gjf-reflow` fixture and gets its expected output
