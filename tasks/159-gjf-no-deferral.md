# Stop deferring input in gjf

Bounded context: `gjf` (`src/gjf/`).

Until the construct tasks are done, `gjf` returns its input unchanged for constructs it cannot format yet (`Unformatted::Deferred`, kept inside `gjf` since task 115). Once they are done, nothing is left to defer. A deferral on a later run of `pipeline::padded_cell` keeps the earlier run's output, so task 154b defers on the first run a markdown fence whose info will take a later run somewhere jeer defers (a backtick fence whose decoded info holds a backtick, which the next run reads as a paragraph, or, since task 154c writes the settled info of one that leaves a lone surrogate in one run, such an info that holds a line terminator by the run that leaves the surrogate, that needs as many runs as Spotless's PaddedCell allows (10), or whose settled info has a backslash before an unpaired surrogate, which the next run would read as an escaped `?`); that check goes once those runs are ported.

## Done when
- `Unformatted::Deferred` is gone, and no input to `gjf::step` comes back unchanged because of a construct jeer does not handle
- A small test runs every fixture input of the `gjf` profile and finds no deferred input
