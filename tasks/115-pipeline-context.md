# Replace the profiles with the pipeline context

Bounded context: the new `pipeline` context (`src/pipeline.rs`, `src/pipeline/`), which replaces `src/profile.rs`. This is the one refactor that sets up the boundaries of docs/design.md §7.1, so it also changes how `steps`, `gjf`, `parser`, `cli` and `src/main.rs` meet it, using the `io` context of task 114; no behavior changes.

Today `src/profile.rs` calls each step by hand and matches `gjf`'s internal `Unformatted`, so every new step or option changes it.

## Done when
- `pipeline` defines `Step` and `Rejection` (an offset in the step's input and a message), composes steps in order, places a rejection at a line and column, and holds the named profiles as lists of steps
- Each step module in `steps` provides a `Step`; `steps::toggle::preserving` wraps a `Step` in a `Step`
- `Rejection` is at an offset in the step's input, or at a line and column the step reports itself
- `gjf::step(Options)` is a `Step` (with `Options::default()`, empty until an option is ported) that parses its input itself and keeps `Unformatted` inside `gjf` (an input it defers comes back unchanged); nothing outside `gjf` names `Unformatted`
- `src/syntax.rs` moves into `parser` as `parser::syntax`
- `src/main.rs` runs `cli::plan`, `io::read`, `cli::run`, `io::write` and holds no `std::io` call of its own
- `src/profile.rs` is gone, `cli` runs a `pipeline` profile, split into `cli::plan` and `cli::run` as §7.1 describes, and every fixture and small test passes unchanged
