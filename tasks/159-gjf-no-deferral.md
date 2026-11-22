# Stop deferring input in gjf

Bounded context: `gjf` (`src/gjf/`).

Until the construct tasks are done, `gjf` returns its input unchanged for constructs it cannot format yet (`Unformatted::Deferred`, kept inside `gjf` since task 115). Once they are done, nothing is left to defer.

## Done when
- `Unformatted::Deferred` is gone, and no input to `gjf::step` comes back unchanged because of a construct jeer does not handle
- A small test runs every fixture input of the `gjf` profile and finds no deferred input
