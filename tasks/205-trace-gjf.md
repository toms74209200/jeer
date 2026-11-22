# Trace gjf

Bounded context: `gjf` (`src/gjf.rs`, `src/gjf/`), calling the `trace` context of task 200.

Design doc §10: the doc_build and print spans.

## Done when
- The context opens the doc_build and print spans through `trace`, and a small test with the feature on sees their records
