# Trace gjf

Bounded context: `gjf` (`src/gjf.rs`, `src/gjf/`), calling the `trace` context of task 200.

Design doc §10: the doc_build and print spans.

## Done when
- The context opens the doc_build and print spans through `trace` with the Doc IR node count and the number of `fits` calls, and a small test with the feature on sees the records
