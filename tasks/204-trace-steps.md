# Trace the steps

Bounded context: `profile` (`src/profile.rs`), where the steps run, calling the `trace` context of task 200.

Design doc §10: a span for each step.

## Done when
- `profile` opens a span around each step it runs through `trace`, and a small test with the feature on sees their records
