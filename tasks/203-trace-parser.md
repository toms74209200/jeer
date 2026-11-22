# Trace parser

Bounded context: `parser` (`src/parser.rs`, `src/parser/`), calling the `trace` context of task 200.

Design doc §10: the parse span.

## Done when
- The context opens the parse span through `trace` with its CST node count, and a small test with the feature on sees the record
