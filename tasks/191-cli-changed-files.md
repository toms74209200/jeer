# Add formatting only changed files

Bounded context: `cli` (`src/cli.rs`, `src/cli/`), calling the changed-file listing of task 190 and the file reading and in-place writes of task 175.

Design doc §6.2: a mode that processes only the Java files changed according to `git diff`, writing back in place, never writing broken output.

## Done when
- A medium test on a temporary git repository formats only the changed files
- Syntax errors leave the file untouched and are reported as in §6.3
