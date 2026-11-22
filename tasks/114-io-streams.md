# Add the io context for stdin and stdout

Bounded context: a new `io` context (`src/io.rs`, `src/io/`), the side-effect module of docs/design.md §7.1.

## Done when
- `io::Request` has a stdin variant, `io::read` reads the requests into `Inputs`, and `io::write` writes stdout and stderr
- A medium test runs `io::read` and `io::write` on a child process's pipes, including a write that fails
