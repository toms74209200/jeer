# Move stdin and stdout into the io context

Bounded context: `io` (`src/io.rs`, `src/io/`), the side-effect module of docs/design.md §7.1; `src/main.rs` only calls it.

## Done when
- `io` reads stdin and writes stdout and stderr; `src/main.rs` calls `io`, then `cli`, then `io`, and holds no `std::io` call of its own
- A medium test runs the binary on stdin and checks its output and exit code, including a write that fails
