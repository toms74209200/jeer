# Build a pipeline from a configuration

Bounded context: `pipeline` (`src/pipeline.rs`, `src/pipeline/`), taking the `config::Spec` of task 170.

## Done when
- `pipeline` composes the steps of a `Spec`, with each selected step and its options in the configured order, and the named profiles are `Spec`s of their own
- Small tests cover each step and option of the `Spec` and their order
