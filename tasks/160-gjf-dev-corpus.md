# Match the oracle on the whole dev corpus

Work through the remaining mismatches of `scripts/corpus-diff.sh dev`, largest class first. For each class, first add a minimal fixture of our own (as a separate commit, with its `skip` marker), then fix it. Split into further task files as needed (tasks policy).

## Done when
- `scripts/corpus-diff.sh dev` reports every file as `match`, except files in `corpus/oracle-bugs.tsv` (path and reason), which a human approved through `shampoo ask`
- Every fixture passes; no `skip` marker remains under `tests/fixtures/gjf/`
- `gjf::format` has no `Unformatted::Deferred` result: the `gjf` profile never falls back to the unchanged input, and the `#[expect(dead_code)]` on `mod gjf` is gone
