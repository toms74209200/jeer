# Read and write files in the io context

Bounded context: `io` (`src/io.rs`, `src/io/`), the side-effect module of docs/design.md §7.1.

The CLI tasks after this one read files, walk directories given as arguments, load the project's configuration and write files back in place. Those side effects live in `io`, beside stdin and stdout (task 114).

## Done when
- `io::Request` covers a file, a directory walked for `.java` files and the configuration file; `io::read` reads them and `io::write` writes a file back only when its content changed
- A medium test on a temporary directory covers each function, including a missing file, an unreadable directory and a write that must not happen
