# Trace steps

Bounded context: `steps` (`src/steps.rs`, `src/steps/`), calling the `trace` context of task 200.

Design doc §10: a span for each step.

## Done when
- The context opens a span for each step through `trace`, and a small test with the feature on sees their records
