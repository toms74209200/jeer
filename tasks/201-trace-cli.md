# Trace cli

Bounded context: `cli` (`src/cli.rs`, `src/cli/`), calling the `trace` context of task 200.

Design doc §10: the process and file spans.

## Done when
- The context opens the process and file spans through `trace`, and a small test with the feature on sees their records
