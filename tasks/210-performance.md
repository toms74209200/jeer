# Meet the performance requirements

Design doc §5 and docs/development.md §6: add a benchmark script comparing against the merge-base on the same machine, with instruction counts as pass/fail and wall time as distributions; then reach the requirements.

## Done when
- Cold single file including process start is under 10 ms (p50) on the dev corpus's median-sized file
- 64 parallel `--check` runs keep p99 under 20 ms per run
- The whole dev corpus formats at least 10x faster than `scripts/oracle.sh` with a warm Gradle daemon
- Time is near-linear in input size on generated pathological inputs (deep nesting, long chains, huge files)
