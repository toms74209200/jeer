# Move file and stream I/O into the io module

Bounded context: `io` (`src/io.rs`, `src/io/`), the side-effect module listed in `scripts/side-effect-modules.txt`; `src/main.rs` only calls it.

The CLI tasks after this one read files, walk directories given as arguments, load the project's configuration and write files back in place. Those side effects live in `io`. Reading stdin and writing stdout move here from `src/main.rs` too, so every read and write goes through one module.

## Done when
- `io` reads a file, walks a directory for `.java` files, reads the configuration file, writes a file back only when its content changed, and reads stdin and writes stdout for `src/main.rs`
- A medium test on a temporary directory covers each function, including a missing file, an unreadable directory and a write that must not happen
