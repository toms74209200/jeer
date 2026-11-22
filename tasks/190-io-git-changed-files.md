# List the Java files git reports as changed

Bounded context: `io` (`src/io.rs`, `src/io/`), the side-effect module listed in `scripts/side-effect-modules.txt`.

Design doc §6.2: the changed-files mode needs the Java files that `git diff` reports, for the working tree and index against HEAD, or against a given revision. This task adds only that side effect; the CLI mode that uses it is task 191.

## Done when
- `io::Request` covers git's changed files for both forms, and `io::read` returns their `.java` paths and contents
- A medium test on a temporary git repository covers modified, added, deleted and renamed files, and paths outside the repository root
