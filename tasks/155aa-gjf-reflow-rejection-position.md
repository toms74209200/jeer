# Report a rejection from formatting the gjf output during reflow where GJF does

Bounded context: `gjf` (`src/gjf.rs`).

With `reflow_long_strings`, GJF's StringWrapper.wrap formats the text it has already formatted (`formatter.formatSource`), so a rejection there is reported at a line of that text. jeer's `reflow_strings` formats `ordered`, the step's own output, and `format_cleaned` can return `Unformatted::Rejected { offset }` with an offset into `ordered`; `settle` returns it as `Position::Offset`, which `pipeline::compose` places against the step's input. The line is then wrong, and `Located::at` can slice past the end of the input or through a char. It is only reachable when formatting fails on GJF's own output. Found in the review of task 155a.

## Done when
- An oracle fixture, added in a fixtures task first, covers a rejection from formatting the step's output during reflow and passes, or the task records why no input reaches it and removes the branch
- A rejection from formatting the step's output during reflow is reported at a line of that output, as GJF reports it
