# Port StringWrapper's first pass and AST check as google-java-format has them

Bounded context: `gjf` (`src/gjf/`), with `pipeline` for the rejection's position.

Task 154 ports StringWrapper's reflow but defers in two places where GJF does more. GJF's first pass formats only the replacement ranges (`formatter.getFormatReplacements(input, ranges)`) and recomputes the replacements on the result; jeer has no range formatting, so it defers the file when formatting the whole input changes it. GJF's AST check compares the javac trees, with string folding, before and after, and throws `FormatterException` ("Something has gone terribly wrong ... would produce code that does not parse" or "would unexpectedly change the AST"), which Spotless reports at `LINE_UNDEFINED`; jeer defers when the result does not reparse. The known input is a long literal joined with a text block that follows it.

## Done when
- Oracle fixtures, added in a fixtures task first, cover the first pass changing a non-idempotent region and the "does not parse" rejection, and pass with their `skip` markers deleted
- The gjf-reflow step rejects with GJF's message where GJF throws, at a position `pipeline` renders as `LINE_UNDEFINED`
