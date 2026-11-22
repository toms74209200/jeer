# Run the reflow profile

Bounded context: `pipeline` (`src/pipeline.rs`, `src/pipeline/`), calling `gjf::step` with the option of task 154.

## Done when
- `pipeline` has a `gjf-reflow` profile, and every `gjf-reflow` fixture passes with its `skip` marker deleted
