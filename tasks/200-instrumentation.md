# Add the instrumentation context

Bounded context: a new `trace` context (`src/trace.rs`), a side-effect module of docs/design.md §7.1.

Design doc §10: spans with the attributes listed there, behind a cargo feature that costs nothing when disabled. Any crate it needs (e.g. tracing) goes through `shampoo ask` first (dependencies policy); without approval, write the spans to a JSON Lines file with std. Tasks 201 to 206 open the spans in each context.

## Done when
- `trace` sets up the counting allocator for bytes allocated, and a span opened and closed through it writes one record with the §10 attributes when the feature is on, and small tests cover the record
- `src/trace.rs` is listed in `scripts/side-effect-modules.txt`
- With the feature off, the release binary has no instrumentation code (size or symbol check in a test)
