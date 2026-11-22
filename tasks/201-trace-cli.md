# Trace the process and its files

Bounded context: `cli` (`src/cli.rs`, `src/cli/`), with `src/main.rs` as the shell of docs/design.md §7.1, calling the `trace` context of task 200.

Design doc §10: the process span covers the shell's four calls, so `src/main.rs` opens it; `cli::run` opens a file span for each input.

## Done when
- `src/main.rs` wraps `cli::plan`, `io::read`, `cli::run` and `io::write` in the process span, and `cli::run` opens a file span per input through `trace`
- A small test of `cli::run` with the feature on sees the file records, and a medium test of the binary sees the process record
