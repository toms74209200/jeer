# Add the benchmark

Bounded context: `scripts/` (no `src` change).

Design doc §5 and docs/development.md §6: a benchmark script comparing against the merge-base on the same machine, with instruction counts as pass/fail and wall time as distributions.

## Done when
- The script reports each requirement of design doc §5 for the current tree and the merge-base
- The requirements that fail become task files named after this one (211, ...), each with its bounded context
