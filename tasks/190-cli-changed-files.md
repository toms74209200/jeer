# Add formatting only changed files

Bounded context: `cli`, with the git calls in a side-effect module of its own (`src/io/`, listed in `scripts/side-effect-modules.txt`).

Design doc §6.2: a mode that processes only the Java files changed according to `git diff` (working tree and index against HEAD, or against a given revision), writing back in place, never writing broken output.

## Done when
- A medium test on a temporary git repository formats only the changed files
- Syntax errors leave the file untouched and are reported as in §6.3
