# Triage the remaining dev corpus mismatches

Bounded context: `tasks/` and `tests/fixtures/` (no `src` change).

After the construct tasks, classify what `scripts/corpus-diff.sh dev` still reports by the first differing construct. Each class becomes a task file of its own sized by the tasks policy, with a minimal fixture of our own added in a spec commit with its `skip` marker.

## Done when
- Every mismatching dev file is assigned to a task file named after this one (161, 162, ...) or listed in `corpus/oracle-bugs.tsv` with a reason approved through `shampoo ask`
- Each new task file names its bounded context and the fixtures that judge it
