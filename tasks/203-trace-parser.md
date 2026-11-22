# Trace parser

Bounded context: `parser` (`src/parser.rs`, `src/parser/`), calling the `trace` context of task 200.

Design doc §10: the parse span.

## Done when
- The context opens the parse span through `trace`, and a small test with the feature on sees their records
