# Reach the Phase 1 milestone

The final goal: Phase 1 of design doc §4 is complete.

## Done when
- `scripts/corpus-diff.sh holdout` reports every file as `match` (or listed in `corpus/oracle-bugs.tsv` with human approval); a holdout mismatch is first reproduced as a fixture of our own, never fixed by looking at the holdout file alone
- `scripts/corpus-diff.sh dev` still matches every file
- Every fixture passes with no `skip` marker left
- The performance requirements of design doc §5 hold, as the benchmark of task 210 reports
- README.md describes installation, the CLI and the configuration
- docs/design.md §4 describes the googleJavaFormat() step as the oracle runs it, including whether long-string reflow is on, as decided in issue #21
- `gjf::format` has no `Unformatted::Deferred` result: the `gjf` profile never falls back to the unchanged input
