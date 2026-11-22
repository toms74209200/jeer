# Add phase instrumentation

Bounded context: a new `trace` context (`src/trace.rs`); the other contexts only open and close its spans.

Design doc §10: spans per phase (process, file, read, lex, parse, each step, doc_build, print, write) with the attributes listed there, behind a cargo feature that costs nothing when disabled. Any crate it needs (e.g. tracing) goes through `shampoo ask` first (dependencies policy); without approval, write the spans to a JSON Lines file with std.

## Done when
- With the feature on, a run writes one record per phase and file with the §10 attributes
- With the feature off, the release binary has no instrumentation code (size or symbol check in a test)
