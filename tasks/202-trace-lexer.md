# Trace lexer

Bounded context: `lexer` (`src/lexer.rs`, `src/lexer/`), calling the `trace` context of task 200.

Design doc §10: the lex span.

## Done when
- The context opens the lex span through `trace`, and a small test with the feature on sees their records
