# Trace io

Bounded context: `io` (`src/io.rs`, `src/io/`), calling the `trace` context of task 200.

Design doc §10: the read and write spans, around the `io` functions of task 169, which also carry stdin and stdout.

## Done when
- The context opens the read and write spans through `trace`, and a small test with the feature on sees their records
- With the feature on, a run writes one record per phase and file with the §10 attributes (process, file, read, lex, parse, each step, doc_build, print, write)
