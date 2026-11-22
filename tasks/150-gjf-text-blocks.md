# Format text blocks as google-java-format does

Bounded context: `gjf` (`src/gjf/`).

`visitTextBlock` and LayoutBuilder's `multilineToken`, which build the solver's `Doc::Lines` (ported in task 105). Spotless runs GJF without `reflowLongStrings`, so long strings are not rewrapped.

## Done when
- `string-text-block`, `string-text-block-in-call` and `modern-text-block-indent` pass with their `skip` markers deleted
- The visitor builds `Doc::Lines`, and with headers (120) and choices (130) done, the `#[expect(dead_code)]` on `mod gjf` is gone
