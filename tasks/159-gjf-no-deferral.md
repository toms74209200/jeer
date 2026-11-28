# Stop deferring input in gjf

Bounded context: `gjf` (`src/gjf/`).

Until the construct tasks are done, `gjf` returns its input unchanged for constructs it cannot format yet (`Unformatted::Deferred`, kept inside `gjf` since task 115). Once they are done, nothing is left to defer. A deferral on a later run of `pipeline::padded_cell` keeps the earlier run's output, so task 154b defers on the first run a markdown fence whose info will take a later run somewhere jeer defers (an unpaired surrogate, task 154c, or a backtick fence whose decoded info holds a backtick, which the next run reads as a paragraph); that check goes once those runs are ported.

## Done when
- `Unformatted::Deferred` is gone, and no input to `gjf::step` comes back unchanged because of a construct jeer does not handle
- A small test runs every fixture input of the `gjf` profile and finds no deferred input
