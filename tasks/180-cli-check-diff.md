# Add --check --format=json and --diff

Bounded context: `cli` (`src/cli.rs`, `src/cli/`), which plans `io::Request`s for the files and directories given (task 169) and reports on what `io` read.

Design doc §6.2: `--check --format=json` reports files needing formatting, changed ranges and syntax error locations; `--diff` prints a unified diff; both over files and directories given as arguments.

## Done when
- Small tests cover the JSON report (including the syntax error location) and the diff output
- Exit codes follow §6.3 for each mode
