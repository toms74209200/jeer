# Add the lint against results callers branch on

The custom lint of docs/development.md §3: reject a function whose result callers inspect (as a condition, a comparison or a match) to recover something the function had already established. Implemented as a dylint lint (it needs call sites), pinned in a script like the other tools. Changing the checks goes through `shampoo ask`.

## Done when
- A dylint library in `tools/` with UI tests showing a flagged and an accepted case for `bool`-returning and `Option`-returning functions
- `.shampoo/rules/branching-results.toml` and its check run it at `edit`, `task` and `pr`, and CI runs it
- It passes on the code base and on every Rust example in `.shampoo/policies/`
